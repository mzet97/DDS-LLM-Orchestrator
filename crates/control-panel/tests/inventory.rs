//! Paridade com `src/control_panel/tests/test_vm_inventory.py`.

use std::collections::BTreeSet;

use control_panel::inventory::{load_inventory, redact};

fn write_inventory(content: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cp-inv-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmpdir");
    let path = dir.join(format!("inv-{}.ini", content.len()));
    std::fs::write(&path, content).expect("write inventory");
    path
}

#[test]
fn load_inventory_parses_groups_and_hosts() {
    let path = write_inventory(
        "\n[orchestrator]\n192.168.1.62 ansible_user=agent ansible_ssh_pass=Secret123\n\n[gpu_amd]\nlocalhost ansible_connection=local ansible_user=mzet\n\n[gpu_nvidia]\n192.168.1.61 ansible_user=agent\n\n[openrouter]\n192.168.1.64 ansible_user=agent ansible_ssh_pass=OtherSecret\n\n[all:vars]\nansible_python_interpreter=/usr/bin/python3\n",
    );
    let inventory = load_inventory(&path);
    assert_eq!(
        inventory.host_names(),
        BTreeSet::from([
            "192.168.1.62".to_string(),
            "localhost".to_string(),
            "192.168.1.61".to_string(),
            "192.168.1.64".to_string()
        ])
    );
    assert!(inventory.group_names().contains("orchestrator"));
    assert!(inventory.group_names().contains("gpu_amd"));
    let by_name = |n: &str| inventory.hosts.iter().find(|h| h.name == n).expect("host");
    assert_eq!(
        by_name("192.168.1.62").ansible_user.as_deref(),
        Some("agent")
    );
    assert_eq!(by_name("localhost").connection.as_deref(), Some("local"));
    assert_eq!(by_name("localhost").groups, vec!["gpu_amd".to_string()]);
}

#[test]
fn sensitive_variables_are_redacted() {
    let path = write_inventory(
        "[orchestrator]\n192.168.1.62 ansible_user=agent ansible_ssh_pass=Secret123\n",
    );
    let inventory = load_inventory(&path);
    let host = &inventory.hosts[0];
    assert_eq!(
        host.variables.get("ansible_ssh_pass").map(|s| s.as_str()),
        Some("Secret123")
    );
    let public = host.to_public();
    assert_eq!(public["variables"]["ansible_ssh_pass"], "******");
    assert_eq!(public["variables"]["ansible_user"], "agent");
}

#[test]
fn missing_inventory_returns_empty() {
    let inventory = load_inventory(std::path::Path::new(
        "/tmp/cp-inv-inexistente-910/inventory.ini",
    ));
    assert!(inventory.hosts.is_empty());
    assert!(inventory.groups.is_empty());
}

#[test]
fn redact_function() {
    let secrets = BTreeSet::from(["Secret123".to_string()]);
    assert_eq!(
        redact("a senha e Secret123 ok", &secrets),
        "a senha e ****** ok"
    );
}

#[test]
fn redact_ignores_short_secrets() {
    let secrets = BTreeSet::from(["ab".to_string()]);
    assert_eq!(redact("ab permanece", &secrets), "ab permanece");
}
