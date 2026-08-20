//! Smoke tests for DDS Security deployment (T-813).
//!
//! These tests require the `security` feature and a local OpenSSL installation.
//! DDS Security performs the authentication handshake per remote participant;
//! because CycloneDDS skips the network handshake for participants created in
//! the same OS process, publisher and subscriber are launched as separate
//! processes via the `security_echo` helper binary.
//!
//! Run with:
//!   cargo test -p dds-dataspace --features dds,security --test security_smoke

#![cfg(feature = "security")]

use std::path::PathBuf;
use std::process::{Command, Stdio};
use std::time::Duration;

fn security_dir() -> PathBuf {
    let mut dir = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    dir.pop(); // crates
    dir.pop(); // workspace root
    dir.push("config");
    dir.push("dds");
    dir.push("security");
    dir
}

fn helper_path() -> PathBuf {
    let mut path = PathBuf::from(env!("CARGO_MANIFEST_DIR"));
    path.pop(); // crates
    path.pop(); // workspace root
    path.push("target");
    path.push("debug");
    path.push("security_echo");
    path
}

fn cyclonedds_test_config() -> PathBuf {
    let mut path = security_dir();
    path.push("cyclonedds-test.xml");
    path
}

fn run_echo(role: &str, domain_id: u32) -> std::process::Child {
    let mut cmd = Command::new(helper_path());
    cmd.arg(role)
        .arg(domain_id.to_string())
        .arg(security_dir())
        .stdout(Stdio::piped())
        .stderr(Stdio::piped());
    // Configuração CycloneDDS local-only: sem ela o handshake de segurança
    // pode falhar em interfaces que não fazem multicast loopback, fazendo o
    // subscriber reenviar handshake_init_message indefinidamente (T-813).
    cmd.env(
        "CYCLONEDDS_URI",
        format!("file://{}", cyclonedds_test_config().display()),
    );
    cmd.spawn().expect("failed to spawn security_echo process")
}

fn unique_domain() -> u32 {
    // Use the current process id modulo a safe range to avoid collisions
    // when tests run concurrently while keeping domains deterministic.
    (std::process::id() % 100) as u32 + 100
}

/// A valid publisher and subscriber in separate processes must discover each
/// other, complete the DDS Security handshake, and exchange a sample.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn secure_participants_exchange_sample() {
    let domain_id = unique_domain();

    let subscriber = run_echo("subscriber", domain_id);
    // Head-start so the subscriber is discovered and the security handshake
    // can start before the publisher writes.
    tokio::time::sleep(Duration::from_secs(2)).await;
    let publisher = run_echo("publisher", domain_id);

    let subscriber_out = subscriber
        .wait_with_output()
        .expect("subscriber process failed");
    let _publisher_out = publisher
        .wait_with_output()
        .expect("publisher process failed");

    assert!(
        subscriber_out.status.success(),
        "subscriber failed: stderr: {}",
        String::from_utf8_lossy(&subscriber_out.stderr)
    );
    let stdout = String::from_utf8_lossy(&subscriber_out.stdout);
    assert!(
        stdout.trim().starts_with("RECEIVED secure-test"),
        "subscriber did not receive expected sample: {}",
        stdout
    );
}

/// A participant with an untrusted/self-signed certificate must be rejected
/// during participant creation.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn intruder_participant_is_rejected() {
    let domain_id = unique_domain();

    let intruder = run_echo("intruder", domain_id);
    let out = intruder
        .wait_with_output()
        .expect("intruder process failed");

    assert!(
        !out.status.success(),
        "intruder with untrusted certificate should be rejected"
    );
}
