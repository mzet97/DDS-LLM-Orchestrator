use super::{fmt_gb, fmt_uptime, probe_build, truncate, Preset, RunnerParams, RunnerState};

#[test]
fn params_build_real_llama_argv() {
    let params = RunnerParams {
        exe: String::from("/usr/bin/llama-server"),
        model: String::from("/m/q.gguf"),
        port: 8081,
        ctx: 1024,
        gpu_layers: 3,
        batch: 64,
        threads: 4,
        bind: String::from("127.0.0.1"),
        health_timeout_secs: 180,
    };
    assert_eq!(
        params.args(),
        vec![
            "-m",
            "/m/q.gguf",
            "--port",
            "8081",
            "--ctx-size",
            "1024",
            "--n-gpu-layers",
            "3",
            "--batch-size",
            "64",
            "--threads",
            "4",
            "--host",
            "127.0.0.1",
        ]
    );
    assert!(params.cli_preview().starts_with("/usr/bin/llama-server -m"));
    assert_eq!(params.base_url(), "http://127.0.0.1:8081");
}

#[test]
fn wildcard_bind_probes_via_loopback() {
    let mut params = RunnerParams::new();
    params.bind = String::from("0.0.0.0");
    params.port = 9099;
    assert_eq!(params.base_url(), "http://127.0.0.1:9099");
}

#[test]
fn presets_apply_documented_values() {
    let mut state = RunnerState::new();
    state.apply_preset(Preset::Latencia);
    assert_eq!(state.params.ctx, 8192);
    assert_eq!(state.params.batch, 1024);
    state.apply_preset(Preset::Precisao);
    assert_eq!(state.params.ctx, 32768);
    assert_eq!(state.params.batch, 512);
}

#[test]
fn validation_catches_empty_and_missing_before_any_effect() {
    let mut state = RunnerState::new();
    state.params.exe.clear();
    assert!(state.validate().is_err());
    state.params.exe = String::from("/bin/echo");
    state.params.model.clear();
    assert!(state.validate().is_err());
    state.params.model = String::from("/definitivamente/ausente.gguf");
    assert!(state.validate().unwrap_err().contains("ausente"));
}

#[test]
fn occupied_port_fails_validation_honestly() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("porta efêmera liga");
    let port = listener.local_addr().expect("addr").port();
    let mut state = RunnerState::new();
    state.params.exe = String::from("/bin/echo");
    state.params.model = String::from("/bin/echo");
    state.params.bind = String::from("127.0.0.1");
    state.params.port = port;
    assert!(state.validate().unwrap_err().contains("ocupada"));
}

#[test]
fn build_probe_reads_real_version_output() {
    // `/bin/true --version` existe e responde (coreutils) — a sonda lê.
    let line = probe_build("/bin/true");
    assert!(
        !line.is_empty() && line != "—",
        "true tem --version: {line}"
    );
    assert_eq!(probe_build("/definitivamente/ausente"), "—");
}

#[test]
fn health_once_measures_real_round_trip() {
    let listener = std::net::TcpListener::bind("127.0.0.1:0").expect("loopback liga");
    let port = listener.local_addr().expect("addr").port();
    std::thread::spawn(move || {
        use std::io::{Read, Write};
        for stream in listener.incoming().flatten().take(2) {
            let mut stream = stream;
            let mut buf = [0u8; 1024];
            let _ = stream.read(&mut buf);
            let _ = stream
                .write_all(b"HTTP/1.1 200 OK\r\ncontent-length: 2\r\nconnection: close\r\n\r\nok");
        }
    });
    let base = format!("http://127.0.0.1:{port}");
    let ms = super::health_once(&base).expect("stub 200 responde");
    assert!(ms < 5_000, "localhost é rápido: {ms}ms");
    assert!(super::health_once("http://127.0.0.1:1").is_err());
}

#[test]
fn helpers_format_without_panic_on_unicode() {
    assert_eq!(fmt_uptime(45), "45s");
    assert_eq!(fmt_uptime(192), "3m 12s");
    assert_eq!(fmt_uptime(7500), "2h 05m");
    assert_eq!(fmt_gb(5_420_000_000), "5.4");
    assert_eq!(truncate("abcdefghij", 4), "abcd…");
    assert_eq!(truncate("ações", 4), "aç…");
}
