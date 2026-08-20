mod common;

use common::TempDir;
use mcp_gateway::tools::FilesystemTool;
use mcp_gateway::{ToolError, ToolRegistry};
use std::sync::atomic::{AtomicBool, Ordering};
use std::sync::Arc;

fn registry_with_fs(root: &std::path::Path) -> ToolRegistry {
    let registry = ToolRegistry::new();
    for tool in FilesystemTool::ops(root).expect("ops") {
        registry.register(tool);
    }
    registry
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn concurrent_symlink_rename_swap_cannot_write_outside_root() {
    let tmp = TempDir::new("fs-swap");
    let outside = tmp
        .path()
        .parent()
        .expect("temporary parent")
        .join(format!("outside-swap-{}", std::process::id()));
    std::fs::create_dir(&outside).expect("outside directory");
    std::fs::write(outside.join("sentinel"), "outside-original").expect("outside sentinel");
    std::fs::create_dir(tmp.path().join("gate")).expect("inside directory");
    std::fs::write(tmp.path().join("gate/sentinel"), "inside").expect("inside sentinel");
    let registry = registry_with_fs(tmp.path());
    let stop = Arc::new(AtomicBool::new(false));
    let attacker_stop = Arc::clone(&stop);
    let root = tmp.path().to_path_buf();
    let outside_for_thread = outside.clone();
    let attacker = std::thread::spawn(move || {
        while !attacker_stop.load(Ordering::Relaxed) {
            if std::fs::rename(root.join("gate"), root.join("parked")).is_ok() {
                let _ = std::os::unix::fs::symlink(&outside_for_thread, root.join("gate"));
                std::thread::yield_now();
                let _ = std::fs::remove_file(root.join("gate"));
                let _ = std::fs::rename(root.join("parked"), root.join("gate"));
            }
        }
    });

    for _ in 0..2_000 {
        let result = registry
            .dispatch(
                FilesystemTool::WRITE_FILE,
                &serde_json::json!({"path": "gate/sentinel", "content": "inside-write"})
                    .to_string(),
            )
            .await;
        if let Err(error) = result {
            assert!(
                matches!(
                    error,
                    ToolError::PathTraversal(_) | ToolError::NotFound(_) | ToolError::Io(_)
                ),
                "unexpected swap error: {error:?}"
            );
        }
    }
    stop.store(true, Ordering::Relaxed);
    attacker.join().expect("attacker joins");
    assert_eq!(
        std::fs::read_to_string(outside.join("sentinel")).expect("outside remains readable"),
        "outside-original"
    );
    eprintln!("swap_attempts=2000 outside_sentinel=unchanged");
    let _ = std::fs::remove_dir_all(outside);
}
