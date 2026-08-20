//! Helper binary for the DDS Security smoke test (T-813).
//!
//! Run one process as publisher and another as subscriber on the same domain.
//! The subscriber exits with success and prints the received server_id when it
//! gets a sample; otherwise it exits with failure after a timeout.
//!
//! Usage:
//!   security_echo publisher <domain_id> <security_dir>
//!   security_echo subscriber <domain_id> <security_dir>

use dds_contract::generated::orchestrator::ServerStatus;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::{DataSpace, SecurityConfig};
use futures::StreamExt;
use std::path::{Path, PathBuf};
use std::time::Duration;
use tokio::time::timeout;

fn security_config(dir: &Path, role: &str) -> SecurityConfig {
    let p = |name: &str| dir.join(name).to_string_lossy().into_owned();
    let mut config = SecurityConfig::new()
        .identity_ca(p("identity_ca_cert.pem"))
        .governance(p("governance.p7s"))
        .permissions_ca(p("permissions_ca_cert.pem"));

    match role {
        "publisher" => {
            config = config
                .identity_certificate(p("publisher_cert.pem"))
                .identity_private_key(p("publisher_key.pem"))
                .permissions(p("permissions_publisher.p7s"))
        }
        "subscriber" => {
            config = config
                .identity_certificate(p("subscriber_cert.pem"))
                .identity_private_key(p("subscriber_key.pem"))
                .permissions(p("permissions_subscriber.p7s"))
        }
        "intruder" => {
            config = config
                .identity_certificate(p("intruder_cert.pem"))
                .identity_private_key(p("intruder_key.pem"))
                .permissions(p("permissions_intruder.p7s"))
        }
        _ => panic!("unknown role: {}", role),
    }

    config
}

#[tokio::main]
async fn main() {
    let args: Vec<String> = std::env::args().collect();
    if args.len() != 4 {
        eprintln!(
            "Usage: {} <publisher|subscriber|intruder> <domain_id> <security_dir>",
            args.get(0).map(|s| s.as_str()).unwrap_or("security_echo")
        );
        std::process::exit(2);
    }

    let role = args[1].clone();
    let domain_id: u32 = args[2].parse().expect("domain_id must be a u32");
    let security_dir = PathBuf::from(&args[3]);

    let config = security_config(&security_dir, &role);

    let role_for_spawn = role.clone();
    let space = tokio::task::spawn_blocking(move || {
        DataSpace::new_with_profile_and_security(
            domain_id,
            match role_for_spawn.as_str() {
                "publisher" => DataSpace::STRENGTH_ORCHESTRATOR,
                _ => DataSpace::STRENGTH_CLIENT,
            },
            None,
            Some(config),
        )
    })
    .await
    .expect("spawn_blocking failed");

    let space = match space {
        Ok(s) => s,
        Err(e) => {
            eprintln!("DataSpace creation failed: {}", e);
            std::process::exit(1);
        }
    };

    match role.as_str() {
        "publisher" => {
            // Give the subscriber time to complete the security handshake.
            tokio::time::sleep(Duration::from_secs(2)).await;
            space
                .write_server_status(ServerStatus {
                    server_id: "secure-test".into(),
                    slots_idle: 1,
                    slots_processing: 0,
                    model_loaded: "model".into(),
                    ready: true,
                })
                .await
                .expect("write should succeed");
            // Keep the participant alive a little longer so the sample is sent.
            tokio::time::sleep(Duration::from_secs(2)).await;
        }
        "subscriber" | "intruder" => {
            let mut stream = space.subscribe_server_status();
            match timeout(Duration::from_secs(30), stream.next()).await {
                Ok(Some(status)) => {
                    println!("RECEIVED {}", status.server_id);
                }
                Ok(None) => {
                    eprintln!("stream ended without sample");
                    std::process::exit(1);
                }
                Err(_) => {
                    eprintln!("timeout waiting for sample");
                    std::process::exit(1);
                }
            }
        }
        _ => unreachable!(),
    }
}
