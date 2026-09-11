use axum::http::{header::AUTHORIZATION, HeaderMap};
use sha2::{Digest, Sha256};
use std::{
    collections::BTreeSet,
    fs,
    net::{IpAddr, Ipv4Addr, SocketAddr},
    path::Path,
    time::Duration,
};
use subtle::ConstantTimeEq;

const MIN_TOKEN_BYTES: usize = 32;

#[derive(Clone, Debug)]
pub struct HttpLimits {
    pub body_bytes: usize,
    pub message_count: usize,
    pub message_bytes: usize,
    pub max_tokens: u32,
    pub concurrent_requests: usize,
    pub dds_wait_timeout: Duration,
}

impl Default for HttpLimits {
    fn default() -> Self {
        Self {
            body_bytes: 1_048_576,
            message_count: 64,
            message_bytes: 262_144,
            max_tokens: 8_192,
            concurrent_requests: 32,
            dds_wait_timeout: Duration::from_secs(120),
        }
    }
}

#[derive(Clone)]
struct Credential {
    client_id: String,
    token_digest: [u8; 32],
}

#[derive(Clone)]
pub struct HttpConfig {
    pub(crate) bind_ip: IpAddr,
    pub(crate) port: u16,
    pub(crate) limits: HttpLimits,
    pub(crate) allowed_models: BTreeSet<String>,
    credentials: Vec<Credential>,
}

#[derive(Clone, Debug, Eq, PartialEq)]
pub struct CallerIdentity(pub String);

#[derive(Debug, thiserror::Error)]
pub enum HttpConfigError {
    #[error("external HTTP bind requires --http-expose")]
    ExposureRequired,
    #[error("external HTTP bind requires an authentication file")]
    AuthenticationRequired,
    #[error("external HTTP bind requires at least one allowed model")]
    ModelAllowlistRequired,
    #[error("HTTP limit {0} must be greater than zero")]
    ZeroLimit(&'static str),
    #[error("could not read HTTP authentication file")]
    AuthFileRead(#[source] std::io::Error),
    #[error("HTTP authentication file has unsafe permissions")]
    AuthFilePermissions,
    #[error("invalid HTTP authentication entry on line {0}")]
    InvalidCredential(usize),
    #[error("duplicate HTTP client identity on line {0}")]
    DuplicateIdentity(usize),
    #[error("duplicate HTTP credential on line {0}")]
    DuplicateCredential(usize),
}

impl HttpConfig {
    #[doc = "Validates the HTTP boundary before DDS startup (REQ-704)."]
    pub fn load(
        bind_ip: IpAddr,
        port: u16,
        expose: bool,
        auth_file: Option<&Path>,
        allowed_models: BTreeSet<String>,
        limits: HttpLimits,
    ) -> Result<Self, HttpConfigError> {
        validate_limits(&limits)?;
        let credentials = match auth_file {
            Some(path) => load_credentials(path)?,
            None => Vec::new(),
        };

        if !bind_ip.is_loopback() {
            if !expose {
                return Err(HttpConfigError::ExposureRequired);
            }
            if credentials.is_empty() {
                return Err(HttpConfigError::AuthenticationRequired);
            }
            if allowed_models.is_empty() {
                return Err(HttpConfigError::ModelAllowlistRequired);
            }
        }

        Ok(Self {
            bind_ip,
            port,
            limits,
            allowed_models,
            credentials,
        })
    }

    #[doc = "Maps a bearer credential to a stable caller identity (REQ-704)."]
    pub fn authenticate(&self, headers: &HeaderMap) -> Option<CallerIdentity> {
        if self.credentials.is_empty() && self.bind_ip.is_loopback() {
            return Some(CallerIdentity("local-trusted".to_owned()));
        }

        let bearer = headers
            .get(AUTHORIZATION)?
            .to_str()
            .ok()?
            .strip_prefix("Bearer ")?;
        if bearer.is_empty() {
            return None;
        }

        let supplied: [u8; 32] = Sha256::digest(bearer.as_bytes()).into();
        let mut matched = None;
        for credential in &self.credentials {
            if bool::from(credential.token_digest.ct_eq(&supplied)) {
                matched = Some(CallerIdentity(credential.client_id.clone()));
            }
        }
        matched
    }

    #[doc = "Returns the validated HTTP listen address (REQ-704)."]
    pub const fn socket_addr(&self) -> SocketAddr {
        SocketAddr::new(self.bind_ip, self.port)
    }

    #[cfg(test)]
    pub(crate) fn for_test(
        bind_ip: IpAddr,
        allowed_models: &[&str],
        credentials: &[(&str, &str)],
        limits: HttpLimits,
    ) -> Self {
        Self {
            bind_ip,
            port: 0,
            limits,
            allowed_models: allowed_models
                .iter()
                .map(|model| (*model).to_owned())
                .collect(),
            credentials: credentials
                .iter()
                .map(|(client_id, token)| Credential {
                    client_id: (*client_id).to_owned(),
                    token_digest: Sha256::digest(token.as_bytes()).into(),
                })
                .collect(),
        }
    }
}

impl Default for HttpConfig {
    fn default() -> Self {
        Self {
            bind_ip: IpAddr::V4(Ipv4Addr::LOCALHOST),
            port: 8080,
            limits: HttpLimits::default(),
            allowed_models: BTreeSet::new(),
            credentials: Vec::new(),
        }
    }
}

fn validate_limits(limits: &HttpLimits) -> Result<(), HttpConfigError> {
    for (name, value) in [
        ("body-bytes", limits.body_bytes),
        ("message-count", limits.message_count),
        ("message-bytes", limits.message_bytes),
        ("max-tokens", limits.max_tokens as usize),
        ("concurrent-requests", limits.concurrent_requests),
    ] {
        if value == 0 {
            return Err(HttpConfigError::ZeroLimit(name));
        }
    }
    if limits.dds_wait_timeout.is_zero() {
        return Err(HttpConfigError::ZeroLimit("dds-wait-timeout-ms"));
    }
    Ok(())
}

fn load_credentials(path: &Path) -> Result<Vec<Credential>, HttpConfigError> {
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        let metadata = fs::metadata(path).map_err(HttpConfigError::AuthFileRead)?;
        if metadata.permissions().mode() & 0o077 != 0 {
            return Err(HttpConfigError::AuthFilePermissions);
        }
    }

    let contents = fs::read_to_string(path).map_err(HttpConfigError::AuthFileRead)?;
    let mut credentials = Vec::new();
    let mut identities = BTreeSet::new();
    let mut token_digests = BTreeSet::new();
    for (index, raw_line) in contents.lines().enumerate() {
        let line_number = index + 1;
        if raw_line.is_empty() || raw_line.starts_with('#') {
            continue;
        }
        let Some((client_id, token)) = raw_line.split_once('=') else {
            return Err(HttpConfigError::InvalidCredential(line_number));
        };
        let valid_identity = !client_id.is_empty()
            && client_id.len() <= 128
            && client_id
                .bytes()
                .all(|byte| byte.is_ascii_alphanumeric() || matches!(byte, b'-' | b'_' | b'.'));
        if !valid_identity || token.len() < MIN_TOKEN_BYTES {
            return Err(HttpConfigError::InvalidCredential(line_number));
        }
        if !identities.insert(client_id.to_owned()) {
            return Err(HttpConfigError::DuplicateIdentity(line_number));
        }
        let token_digest: [u8; 32] = Sha256::digest(token.as_bytes()).into();
        if !token_digests.insert(token_digest) {
            return Err(HttpConfigError::DuplicateCredential(line_number));
        }
        credentials.push(Credential {
            client_id: client_id.to_owned(),
            token_digest,
        });
    }
    Ok(credentials)
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn default_bind_is_loopback() {
        let config = HttpConfig::default();
        assert_eq!(config.bind_ip, IpAddr::V4(Ipv4Addr::LOCALHOST));
    }

    #[test]
    fn external_bind_without_exposure_is_rejected() {
        let result = HttpConfig::load(
            IpAddr::V4(Ipv4Addr::UNSPECIFIED),
            8080,
            false,
            None,
            BTreeSet::new(),
            HttpLimits::default(),
        );
        assert!(matches!(result, Err(HttpConfigError::ExposureRequired)));
    }
}
