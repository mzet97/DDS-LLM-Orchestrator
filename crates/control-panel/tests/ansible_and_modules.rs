//! Paridade com `test_ansible_runner.py`, `test_vm_manager.py`,
//! `test_hardware_metrics.py` e os testes (quebrados no Python) de chat.

use std::collections::BTreeSet;

use control_panel::ansible::{list_actions, AnsibleError, AnsibleRunner};
use control_panel::chat::{
    build_messages_json, build_transcription_payload, ChatError, ChatFile, Specialization,
};
use control_panel::hardware::{
    cpu_metrics, hardware_summary, memory_metrics, parse_float, parse_int,
};
use control_panel::test_runner::discover;

fn tmp_root(tag: &str) -> std::path::PathBuf {
    let dir = std::env::temp_dir().join(format!("cp-ansible-{tag}-{}", std::process::id()));
    std::fs::create_dir_all(&dir).expect("tmpdir");
    dir
}

#[test]
fn command_uses_list_not_shell() {
    let root = tmp_root("list");
    std::fs::write(root.join("status.yml"), "---\n").expect("playbook");
    let runner = AnsibleRunner::new(root.clone(), root.join("inv.ini"));
    let playbook = runner.resolve_playbook("status.yml").expect("resolve");
    let cmd = runner
        .build_command(&playbook, None, &BTreeSet::new())
        .expect("cmd");
    assert_eq!(cmd[0], "ansible-playbook");
    assert!(!cmd.iter().any(|a| a == "--limit"));
    // Lista, nunca string de shell:
    assert!(cmd.iter().all(|a| !a.contains(';') && !a.contains("&&")));
}

#[test]
fn playbook_path_escape_is_rejected() {
    let root = tmp_root("escape");
    let runner = AnsibleRunner::new(root.clone(), root.join("inv.ini"));
    assert!(matches!(
        runner.resolve_playbook("../etc/passwd"),
        Err(AnsibleError::PlaybookEscape(_))
    ));
}

#[test]
fn missing_playbook_raises() {
    let root = tmp_root("missing");
    let runner = AnsibleRunner::new(root.clone(), root.join("inv.ini"));
    assert!(matches!(
        runner.resolve_playbook("missing.yml"),
        Err(AnsibleError::PlaybookMissing(_))
    ));
}

#[test]
fn unknown_action_and_invalid_limit_are_rejected() {
    let root = tmp_root("limits");
    std::fs::write(root.join("status.yml"), "---\n").expect("playbook");
    let runner = AnsibleRunner::new(root.clone(), root.join("inv.ini"));
    let allowed = BTreeSet::from(["localhost".to_string()]);
    assert!(matches!(
        runner.start("desconhecida", None, &BTreeSet::new(), &allowed),
        Err(AnsibleError::UnknownAction(_))
    ));
    let playbook = runner.resolve_playbook("status.yml").expect("resolve");
    assert!(matches!(
        runner.build_command(&playbook, Some("nao-existe"), &allowed),
        Err(AnsibleError::InvalidLimit(_))
    ));
    assert!(runner
        .build_command(&playbook, Some("localhost"), &allowed)
        .is_ok());
}

#[test]
fn manager_lists_actions() {
    let actions = list_actions();
    let ids: Vec<&str> = actions.iter().map(|a| a.id.as_str()).collect();
    assert!(ids.contains(&"status"));
    assert!(ids.contains(&"deploy"));
    assert!(
        actions
            .iter()
            .find(|a| a.id == "stop")
            .expect("stop")
            .dangerous
    );
}

#[test]
fn cpu_and_memory_have_required_keys() {
    let cpu = cpu_metrics();
    for key in [
        "count_logical",
        "count_physical",
        "percent",
        "load_average_1m",
    ] {
        assert!(cpu.get(key).is_some(), "cpu sem {key}");
    }
    let mem = memory_metrics();
    for key in ["total_mb", "available_mb", "used_mb", "percent"] {
        assert!(mem.get(key).is_some(), "mem sem {key}");
    }
    let summary = hardware_summary();
    assert!(
        summary.get("cpu").is_some()
            && summary.get("memory").is_some()
            && summary.get("gpus").is_some()
    );
}

#[test]
fn parse_helpers() {
    assert_eq!(parse_float("42.5"), Some(42.5));
    assert_eq!(parse_float("10 %"), Some(10.0));
    assert_eq!(parse_float("n/a"), None);
    assert_eq!(parse_int("7"), Some(7));
    assert_eq!(parse_int("x"), None);
}

#[test]
fn build_text_message() {
    let json = build_messages_json("ola", Specialization::Text, &[]).expect("msg");
    let value: serde_json::Value = serde_json::from_str(&json).expect("json");
    assert_eq!(value[0]["content"][0]["text"], "ola");
}

#[test]
fn build_vision_message_with_image() {
    let file = ChatFile {
        name: "a.png".into(),
        mime_type: "image/png".into(),
        data_base64: "AA==".into(),
    };
    let json = build_messages_json("veja", Specialization::Vision, &[file]).expect("msg");
    assert!(json.contains("image_url"));
    assert!(json.contains("data:image/png;base64,AA=="));
}

#[test]
fn build_transcription_payload_requires_audio() {
    let audio = ChatFile {
        name: "a.wav".into(),
        mime_type: "audio/wav".into(),
        data_base64: "AA==".into(),
    };
    let json = build_transcription_payload("transcreva", &[audio]).expect("payload");
    assert!(json.contains("transcription"));
    assert!(matches!(
        build_transcription_payload("x", &[]),
        Err(ChatError::UnsupportedFileType(_))
    ));
}

#[test]
fn build_message_with_unsupported_file_type() {
    let file = ChatFile {
        name: "a.pdf".into(),
        mime_type: "application/pdf".into(),
        data_base64: "AA==".into(),
    };
    assert!(matches!(
        build_messages_json("ola", Specialization::Text, &[file]),
        Err(ChatError::UnsupportedFileType(_))
    ));
}

#[test]
fn test_runner_discovers_items() {
    let root = tmp_root("discover");
    std::fs::write(root.join("test_alpha.py"), "\"\"\"Teste alpha.\"\"\"\n").expect("file");
    std::fs::write(root.join("smoke_beta.py"), "x = 1\n").expect("file");
    std::fs::write(root.join("helper.py"), "x = 1\n").expect("file");
    let items = discover(&root);
    assert_eq!(items.len(), 2);
    assert!(items.iter().any(|i| i.category == "smoke"));
    assert!(items.iter().any(|i| i.description.contains("Teste alpha")));
}
