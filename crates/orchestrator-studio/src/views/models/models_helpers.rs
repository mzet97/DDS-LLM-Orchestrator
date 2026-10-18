//! Parte de `views::models` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}

/// SHA truncado para a tabela (32 hex + …; completo no hover).
pub(super) fn short_sha(sha: &str) -> String {
    if sha.len() > 34 {
        format!("{}…", &sha[..32])
    } else {
        String::from(sha)
    }
}

/// GiB com 2 casas ("4.68 GB" do mockup usa GB decimal — aqui GB=1e9).
pub(super) fn fmt_gb2(bytes: u64) -> String {
    format!("{:.2} GB", bytes as f64 / 1_000_000_000.0)
}

/// Segundos → HH:MM:SS (ETA do mockup).
pub(super) fn fmt_hhmmss(secs: u64) -> String {
    format!(
        "{:02}:{:02}:{:02}",
        secs / 3600,
        (secs / 60) % 60,
        secs % 60
    )
}

/// Hostname da máquina (chip STORAGE-HOST); "—" se indisponível.
pub(super) fn hostname() -> String {
    #[cfg(target_os = "linux")]
    {
        let mut buf = [0 as libc::c_char; 256];
        // SAFETY: gethostname escreve no máximo o tamanho dado; NUL garantido.
        let ok = unsafe { libc::gethostname(buf.as_mut_ptr(), buf.len()) == 0 };
        if ok {
            // SAFETY: buf é NUL-terminado pelo gethostname em sucesso.
            let cstr = unsafe { std::ffi::CStr::from_ptr(buf.as_ptr()) };
            if let Ok(name) = cstr.to_str() {
                if !name.is_empty() {
                    return String::from(name);
                }
            }
        }
        String::from("—")
    }
    #[cfg(not(target_os = "linux"))]
    {
        String::from("—")
    }
}

/// Filesystem do caminho ("ext4 · rw,noatime"); `None` se ilegível.
/// Tipo vem do `statfs` (magia), flags do `statvfs` (o `statfs` da libc
/// não expõe `f_flags` no x86_64).
pub(super) fn fs_info(path: &std::path::Path) -> Option<String> {
    #[cfg(target_os = "linux")]
    {
        let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
        let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: statfs preenche a struct zerada; caminho NUL válido.
        if unsafe { libc::statfs(cpath.as_ptr(), &mut stat) } != 0 {
            return None;
        }
        let fstype = match stat.f_type as u64 as u32 {
            0xEF53 => "ext4",
            0x01021994 => "tmpfs",
            0x9123683E => "btrfs",
            0x58465342 => "xfs",
            0x794C7630 => "overlayfs",
            0x6969 => "nfs",
            0xFF534D42 => "smb",
            0xFE534D42 => "smb2",
            0x65735546 => "fuse",
            _ => "fs",
        };
        let mut vfs: libc::statvfs = unsafe { std::mem::zeroed() };
        // SAFETY: idem; flags de montagem (ST_RDONLY/ST_NOATIME).
        if unsafe { libc::statvfs(cpath.as_ptr(), &mut vfs) } != 0 {
            return Some(String::from(fstype));
        }
        let flags = vfs.f_flag;
        let rw = if flags & libc::ST_RDONLY != 0 {
            "ro"
        } else {
            "rw"
        };
        let noatime = if flags & libc::ST_NOATIME != 0 {
            ",noatime"
        } else {
            ""
        };
        Some(format!("{fstype} · {rw}{noatime}"))
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}

/// Bytes totais do filesystem do caminho (`None` se ilegível).
pub(super) fn fs_total_bytes(path: &std::path::Path) -> Option<u64> {
    #[cfg(target_os = "linux")]
    {
        let cpath = std::ffi::CString::new(path.as_os_str().as_encoded_bytes()).ok()?;
        let mut stat: libc::statfs = unsafe { std::mem::zeroed() };
        // SAFETY: idem fs_info.
        if unsafe { libc::statfs(cpath.as_ptr(), &mut stat) } != 0 {
            return None;
        }
        let block = if stat.f_frsize > 0 {
            stat.f_frsize as u64
        } else {
            stat.f_bsize as u64
        };
        Some(stat.f_blocks as u64 * block)
    }
    #[cfg(not(target_os = "linux"))]
    {
        let _ = path;
        None
    }
}
