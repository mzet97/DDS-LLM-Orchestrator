//! Linux directory-fd sandbox with atomic no-symlink resolution.

use crate::error::ToolError;
use rustix::fd::OwnedFd;
use rustix::fs::{self, FileType, Mode, OFlags, ResolveFlags};
use std::io::{Read, Write};
use std::path::{Component, Path, PathBuf};

pub(super) struct SandboxRoot {
    display_path: PathBuf,
    root: OwnedFd,
}

impl SandboxRoot {
    pub(super) fn open(path: &Path) -> Result<Self, ToolError> {
        std::fs::create_dir_all(path)?;
        let display_path = path.canonicalize()?;
        let root = fs::open(
            &display_path,
            OFlags::RDONLY | OFlags::DIRECTORY | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            Mode::empty(),
        )
        .map_err(io_error)?;
        Ok(Self { display_path, root })
    }

    pub(super) fn display_path(&self) -> &Path {
        &self.display_path
    }

    pub(super) fn read(&self, raw: &str, max: u64) -> Result<String, ToolError> {
        let fd = self.open_beneath(raw, OFlags::RDONLY, Mode::empty())?;
        let stat = fs::fstat(&fd).map_err(io_error)?;
        if FileType::from_raw_mode(stat.st_mode) == FileType::Directory {
            return Err(ToolError::InvalidArguments(format!(
                "'{raw}' é um diretório (use filesystem.list_directory)"
            )));
        }
        let size = u64::try_from(stat.st_size).unwrap_or(u64::MAX);
        if size > max {
            return Err(ToolError::TooLarge { size, max });
        }
        let mut bytes = Vec::with_capacity(usize::try_from(size).unwrap_or(0));
        std::fs::File::from(fd)
            .take(max.saturating_add(1))
            .read_to_end(&mut bytes)?;
        let actual = u64::try_from(bytes.len()).unwrap_or(u64::MAX);
        if actual > max {
            return Err(ToolError::TooLarge { size: actual, max });
        }
        Ok(String::from_utf8_lossy(&bytes).into_owned())
    }

    pub(super) fn write(&self, raw: &str, content: &[u8]) -> Result<(), ToolError> {
        let fd = self.open_beneath(
            raw,
            OFlags::WRONLY | OFlags::CREATE | OFlags::TRUNC,
            Mode::RUSR | Mode::WUSR,
        )?;
        let mut file = std::fs::File::from(fd);
        file.write_all(content)?;
        file.sync_all()?;
        Ok(())
    }

    pub(super) fn list(&self, raw: &str, max: usize) -> Result<String, ToolError> {
        let fd = self.open_beneath(raw, OFlags::RDONLY | OFlags::DIRECTORY, Mode::empty())?;
        let mut dir = fs::Dir::read_from(&fd).map_err(io_error)?;
        let mut entries = Vec::new();
        while let Some(entry) = dir.read() {
            let entry = entry.map_err(io_error)?;
            let name = entry.file_name().to_string_lossy();
            if name == "." || name == ".." || name == ".mcp-claims" {
                continue;
            }
            let tag = if entry.file_type() == FileType::Directory {
                "[DIR]"
            } else {
                "[FILE]"
            };
            entries.push(format!("{tag} {name}"));
        }
        entries.sort();
        if entries.len() > max {
            let omitted = entries.len() - max;
            entries.truncate(max);
            entries.push(format!("... ({omitted} entradas omitidas)"));
        }
        Ok(entries.join("\n"))
    }

    fn open_beneath(&self, raw: &str, flags: OFlags, mode: Mode) -> Result<OwnedFd, ToolError> {
        validate_relative(raw)?;
        fs::openat2(
            &self.root,
            raw,
            flags | OFlags::CLOEXEC | OFlags::NOFOLLOW,
            mode,
            ResolveFlags::BENEATH | ResolveFlags::NO_SYMLINKS | ResolveFlags::NO_MAGICLINKS,
        )
        .map_err(|error| map_open_error(raw, error))
    }
}

fn validate_relative(raw: &str) -> Result<(), ToolError> {
    if raw.is_empty() {
        return Err(ToolError::InvalidArguments("path vazio".into()));
    }
    for component in Path::new(raw).components() {
        match component {
            Component::Normal(name) if name == ".mcp-claims" => {
                return Err(ToolError::PathTraversal(raw.to_owned()))
            }
            Component::ParentDir | Component::RootDir | Component::Prefix(_) => {
                return Err(ToolError::PathTraversal(raw.to_owned()))
            }
            Component::CurDir | Component::Normal(_) => {}
        }
    }
    Ok(())
}

fn map_open_error(raw: &str, error: rustix::io::Errno) -> ToolError {
    match error {
        rustix::io::Errno::NOENT => ToolError::NotFound(raw.to_owned()),
        rustix::io::Errno::LOOP | rustix::io::Errno::XDEV => {
            ToolError::PathTraversal(raw.to_owned())
        }
        other => ToolError::Io(io_error(other)),
    }
}

fn io_error(error: rustix::io::Errno) -> std::io::Error {
    std::io::Error::from_raw_os_error(error.raw_os_error())
}
