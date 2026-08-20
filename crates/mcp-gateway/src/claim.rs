//! Durable, atomic ownership claims for exactly-once tool-call dispatch.

use dashmap::DashMap;
use rustix::fd::OwnedFd;
use rustix::fs::{self, Mode, OFlags, ResolveFlags};
use std::io::Write;
use std::path::Path;

const MAX_CALL_ID_BYTES: usize = 256;

/// Stable gateway identity recorded with a claim.
#[derive(Clone, Debug, Eq, PartialEq)]
pub struct OwnerId(String);

impl OwnerId {
    /// Parses a non-empty, bounded identity.
    pub fn parse(raw: &str) -> Result<Self, ClaimError> {
        if raw.is_empty() || raw.len() > MAX_CALL_ID_BYTES {
            return Err(ClaimError::InvalidOwner);
        }
        Ok(Self(raw.to_owned()))
    }

    fn as_str(&self) -> &str {
        &self.0
    }

    pub(crate) fn generated(process: u32, sequence: u64) -> Self {
        Self(format!("gateway-{process}-{sequence}"))
    }
}

/// Outcome of an atomic ownership attempt.
#[derive(Clone, Copy, Debug, Eq, PartialEq)]
pub enum ClaimDecision {
    /// This caller created the permanent claim and may dispatch once.
    Won,
    /// A claim already exists; ownership never transfers implicitly.
    AlreadyClaimed,
}

/// Typed claim-store failures.
#[derive(Debug, thiserror::Error)]
pub enum ClaimError {
    /// The untrusted call identifier is empty or oversized.
    #[error("call_id must contain 1..={MAX_CALL_ID_BYTES} bytes")]
    InvalidCallId,
    /// The configured gateway identity is empty or oversized.
    #[error("owner_id must contain 1..={MAX_CALL_ID_BYTES} bytes")]
    InvalidOwner,
    /// Persistent claim storage failed.
    #[error("claim storage: {0}")]
    Io(#[from] std::io::Error),
}

/// Atomic idempotency boundary used before every side effect (REQ-706).
pub trait ClaimStore: Send + Sync {
    /// Permanently claims `call_id`; a retry never becomes a new winner.
    fn try_claim(&self, call_id: &str, owner: &OwnerId) -> Result<ClaimDecision, ClaimError>;
}

/// Process-local claim store for unit tests and embedded single-process users.
#[derive(Default)]
pub struct MemoryClaimStore {
    claims: DashMap<String, OwnerId>,
}

impl ClaimStore for MemoryClaimStore {
    fn try_claim(&self, call_id: &str, owner: &OwnerId) -> Result<ClaimDecision, ClaimError> {
        validate_call_id(call_id)?;
        match self.claims.entry(call_id.to_owned()) {
            dashmap::mapref::entry::Entry::Occupied(_) => Ok(ClaimDecision::AlreadyClaimed),
            dashmap::mapref::entry::Entry::Vacant(entry) => {
                entry.insert(owner.clone());
                Ok(ClaimDecision::Won)
            }
        }
    }
}

/// Cross-process claims persisted as atomically-created files beneath a held directory fd.
pub struct FileClaimStore {
    root: OwnedFd,
}

impl FileClaimStore {
    /// Opens a trusted claim directory. The fd pins it against rename/symlink swaps.
    pub fn new(path: &Path) -> Result<Self, ClaimError> {
        std::fs::create_dir_all(path)?;
        let root = fs::open(
            path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(errno_to_io)?;
        Ok(Self { root })
    }
}

impl ClaimStore for FileClaimStore {
    fn try_claim(&self, call_id: &str, owner: &OwnerId) -> Result<ClaimDecision, ClaimError> {
        validate_call_id(call_id)?;
        let name = hex_name(call_id.as_bytes());
        let opened = fs::openat2(
            &self.root,
            name,
            OFlags::WRONLY | OFlags::CREATE | OFlags::EXCL | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::RUSR | Mode::WUSR,
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        );
        let fd = match opened {
            Ok(fd) => fd,
            Err(error) if error == rustix::io::Errno::EXIST => {
                return Ok(ClaimDecision::AlreadyClaimed)
            }
            Err(error) => return Err(ClaimError::Io(errno_to_io(error))),
        };
        let mut file = std::fs::File::from(fd);
        file.write_all(owner.as_str().as_bytes())?;
        file.sync_all()?;
        Ok(ClaimDecision::Won)
    }
}

fn validate_call_id(call_id: &str) -> Result<(), ClaimError> {
    if call_id.is_empty() || call_id.len() > MAX_CALL_ID_BYTES {
        Err(ClaimError::InvalidCallId)
    } else {
        Ok(())
    }
}

fn hex_name(bytes: &[u8]) -> String {
    const HEX: &[u8; 16] = b"0123456789abcdef";
    let mut output = String::with_capacity(bytes.len() * 2);
    for byte in bytes {
        output.push(char::from(HEX[usize::from(byte >> 4)]));
        output.push(char::from(HEX[usize::from(byte & 0x0f)]));
    }
    output
}

fn errno_to_io(error: rustix::io::Errno) -> std::io::Error {
    std::io::Error::from_raw_os_error(error.raw_os_error())
}

#[cfg(test)]
mod tests {
    use super::*;
    use std::sync::Arc;

    #[test]
    fn concurrent_memory_claim_has_one_winner() {
        let store = Arc::new(MemoryClaimStore::default());
        let owner = OwnerId::parse("gateway").expect("owner");
        let threads: Vec<_> = (0..32)
            .map(|_| {
                let store = Arc::clone(&store);
                let owner = owner.clone();
                std::thread::spawn(move || store.try_claim("call", &owner).expect("claim"))
            })
            .collect();
        let winners = threads
            .into_iter()
            .map(|thread| thread.join().expect("thread"))
            .filter(|decision| *decision == ClaimDecision::Won)
            .count();
        assert_eq!(winners, 1);
    }

    #[test]
    fn call_ids_are_encoded_as_plain_filenames() {
        assert_eq!(
            hex_name(b"../prompt\nignore"),
            "2e2e2f70726f6d70740a69676e6f7265"
        );
    }
}
