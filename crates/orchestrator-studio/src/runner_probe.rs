//! Sondas e formatadores do runner (extraídos de runner.rs).

use std::io::{BufReader, Read};
use std::process::{Command, Stdio};

pub fn detect_exe() -> Option<String> {
    let mut candidates = vec![
        String::from("/usr/bin/llama-server"),
        String::from("/usr/local/bin/llama-server"),
    ];
    if let Ok(path) = std::env::var("PATH") {
        for dir in path.split(':') {
            candidates.push(format!("{dir}/llama-server"));
        }
    }
    candidates.into_iter().find(|exe| is_executable(exe))
}

/// Verdadeiro quando o caminho existe e tem bit de execução (unix).
pub(crate) fn is_executable(path: &str) -> bool {
    let candidate = std::path::Path::new(path);
    if !candidate.is_file() {
        return false;
    }
    #[cfg(unix)]
    {
        use std::os::unix::fs::PermissionsExt;
        candidate
            .metadata()
            .is_ok_and(|meta| meta.permissions().mode() & 0o111 != 0)
    }
    #[cfg(not(unix))]
    {
        true
    }
}

/// Acelerador via DRI real: `(rótulo, vram_usada_B?, vram_total_B?)`.
/// `None` = sem GPU DRI (CPU) ou sem hwmon (VRAM indisponível, nunca 0%).
#[must_use]
pub fn probe_gpu() -> (Option<String>, Option<(u64, u64)>) {
    let mut label = None;
    let mut vram = None;
    let Ok(drm) = std::fs::read_dir("/sys/class/drm") else {
        return (None, None);
    };
    let mut cards: Vec<String> = drm
        .flatten()
        .map(|entry| entry.file_name().to_string_lossy().into_owned())
        .filter(|name| name.starts_with("card") && !name.contains('-'))
        .collect();
    cards.sort();
    for card in cards {
        let base = format!("/sys/class/drm/{card}/device");
        let hwmon = std::fs::read_dir(format!("{base}/hwmon"))
            .ok()
            .and_then(|mut dirs| dirs.next())
            .and_then(|entry| entry.ok())
            .map(|entry| entry.path().join("name"));
        let name = hwmon
            .as_ref()
            .and_then(|path| std::fs::read_to_string(path).ok())
            .map(|raw| raw.trim().to_owned())
            .filter(|raw| !raw.is_empty());
        let total = std::fs::read_to_string(format!("{base}/mem_info_vram_total"))
            .ok()
            .and_then(|raw| raw.trim().parse::<u64>().ok());
        let used = std::fs::read_to_string(format!("{base}/mem_info_vram_used"))
            .ok()
            .and_then(|raw| raw.trim().parse::<u64>().ok());
        if name.is_some() || total.is_some() {
            label = Some(format!("{} ({card})", name.as_deref().unwrap_or("gpu-dri")));
            if let (Some(used), Some(total)) = (used, total) {
                vram = Some((used, total));
            }
            break;
        }
    }
    (label, vram)
}

/// Uma sonda `/health` (ms medidos). `Ok` = 200.
pub fn health_once(base_url: &str) -> Result<u64, String> {
    let url = base_url.trim_end_matches('/');
    let started = std::time::Instant::now();
    let client = reqwest::blocking::Client::builder()
        .timeout(std::time::Duration::from_secs(3))
        .build()
        .map_err(|err| err.to_string())?;
    client
        .get(format!("{url}/health"))
        .send()
        .map_err(|err| err.to_string())?
        .error_for_status()
        .map_err(|err| err.to_string())?;
    Ok(started.elapsed().as_millis() as u64)
}

/// Uptime humano ("45s", "3m 12s", "2h 05m").
#[must_use]
pub fn fmt_uptime(secs: u64) -> String {
    if secs < 60 {
        format!("{secs}s")
    } else if secs < 3600 {
        format!("{}m {:02}s", secs / 60, secs % 60)
    } else {
        format!("{}h {:02}m", secs / 3600, (secs % 3600) / 60)
    }
}

/// Gibibytes com 1 casa ("5.4").
#[must_use]
pub fn fmt_gb(bytes: u64) -> String {
    format!("{:.1}", bytes as f64 / 1_000_000_000.0)
}

/// Identidade de build do executável (probe por execução curta).
pub(crate) fn probe_build(exe: &str) -> String {
    let mut child = match Command::new(exe)
        .arg("--version")
        .stdin(Stdio::null())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped())
        .spawn()
    {
        Ok(child) => child,
        Err(_) => return String::from("—"),
    };
    let deadline = std::time::Instant::now() + std::time::Duration::from_secs(3);
    loop {
        match child.try_wait() {
            Ok(Some(_)) => break,
            Ok(None) if std::time::Instant::now() < deadline => {
                std::thread::sleep(std::time::Duration::from_millis(50));
            }
            _ => {
                let _ = child.kill();
                return String::from("—");
            }
        }
    }
    let mut output = String::new();
    if let Some(stdout) = child.stdout.take() {
        let _ = BufReader::new(stdout).read_to_string(&mut output);
    }
    if let Some(stderr) = child.stderr.take() {
        let _ = BufReader::new(stderr).read_to_string(&mut output);
    }
    let lines: Vec<&str> = output.lines().collect();
    if let Some(device) = lines.iter().find(|line| line.contains("Device ")) {
        return truncate(device.trim(), 90);
    }
    if let Some(version) = lines.iter().find(|line| line.contains("version:")) {
        return truncate(version.trim(), 90);
    }
    lines
        .iter()
        .map(|line| line.trim())
        .find(|line| !line.is_empty())
        .map_or_else(|| String::from("—"), |line| truncate(line, 90))
}

pub(crate) fn truncate(raw: &str, max: usize) -> String {
    if raw.len() <= max {
        return String::from(raw);
    }
    let end = raw
        .char_indices()
        .map(|(i, _)| i)
        .take_while(|i| *i <= max)
        .last()
        .unwrap_or(0);
    format!("{}…", raw[..end].trim_end())
}
