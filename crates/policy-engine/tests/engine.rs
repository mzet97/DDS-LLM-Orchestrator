//! Testes do motor local — porte fiel de `policy_engine.py`:
//! nível de segurança, whitelist/blacklist, rate limit (janela de 60 s)
//! e defaults do construtor Python.

use dds_contract::generated::dds_llm_orchestrator::ToolCallRequest;
use policy_engine::engine::{LocalPolicyEngine, SecurityLevel, ToolCallStatus};

fn req(tool_name: &str, security_level: i32, request_id: &str) -> ToolCallRequest {
    ToolCallRequest {
        tool_name: tool_name.into(),
        security_level,
        request_id: "correlation-id".into(),
        requester_id: request_id.into(),
        ..Default::default()
    }
}

#[test]
fn default_igual_ao_python() {
    // PolicyEngine(): CONFIDENTIAL, sem listas, 60/min.
    let engine = LocalPolicyEngine::default();
    assert_eq!(
        engine.evaluate(&req("qualquer", 0, "r1")),
        ToolCallStatus::Allowed
    );
    assert_eq!(
        engine.evaluate(&req("qualquer", 2, "r1")),
        ToolCallStatus::Allowed
    );
    assert_eq!(
        engine.evaluate(&req("qualquer", 3, "r1")),
        ToolCallStatus::Denied
    );
}

#[test]
fn security_level_acima_do_max_nega() {
    let engine = LocalPolicyEngine::new(
        SecurityLevel::Internal,
        [] as [&str; 0],
        [] as [&str; 0],
        60,
    );
    assert_eq!(engine.evaluate(&req("t", 0, "r")), ToolCallStatus::Allowed);
    assert_eq!(engine.evaluate(&req("t", 1, "r")), ToolCallStatus::Allowed);
    assert_eq!(engine.evaluate(&req("t", 2, "r")), ToolCallStatus::Denied);
    assert_eq!(engine.evaluate(&req("t", -1, "r")), ToolCallStatus::Denied);
    assert_eq!(engine.evaluate(&req("t", 99, "r")), ToolCallStatus::Denied);
}

#[test]
fn whitelist_bloqueia_fora_da_lista() {
    let engine = LocalPolicyEngine::new(
        SecurityLevel::Confidential,
        ["filesystem.read_file", "filesystem.list_directory"],
        [] as [&str; 0],
        60,
    );
    assert_eq!(
        engine.evaluate(&req("filesystem.read_file", 0, "r")),
        ToolCallStatus::Allowed
    );
    assert_eq!(
        engine.evaluate(&req("filesystem.write_file", 0, "r")),
        ToolCallStatus::Denied
    );
}

#[test]
fn whitelist_vazia_permite_qualquer_tool() {
    let engine = LocalPolicyEngine::new(
        SecurityLevel::Confidential,
        [] as [&str; 0],
        [] as [&str; 0],
        60,
    );
    assert_eq!(
        engine.evaluate(&req("qualquer.tool", 0, "r")),
        ToolCallStatus::Allowed
    );
}

#[test]
fn blacklist_nega_mesmo_na_whitelist() {
    // No Python a whitelist é checada antes da blacklist; uma tool na
    // blacklist E fora da whitelist já cai na whitelist. Aqui ela está nas
    // duas: passa pela whitelist e cai na blacklist.
    let engine = LocalPolicyEngine::new(
        SecurityLevel::Confidential,
        ["tool.perigosa", "tool.ok"],
        ["tool.perigosa"],
        60,
    );
    assert_eq!(
        engine.evaluate(&req("tool.perigosa", 0, "r")),
        ToolCallStatus::Denied
    );
    assert_eq!(
        engine.evaluate(&req("tool.ok", 0, "r")),
        ToolCallStatus::Allowed
    );
}

#[test]
fn precedencia_security_antes_das_listas() {
    let engine = LocalPolicyEngine::new(SecurityLevel::Public, ["tool.x"], ["tool.x"], 60);
    // security_level acima do máximo → DENIED antes de olhar as listas.
    assert_eq!(
        engine.evaluate(&req("tool.x", 1, "r")),
        ToolCallStatus::Denied
    );
}

#[test]
fn rate_limit_por_agente() {
    let engine = LocalPolicyEngine::new(
        SecurityLevel::Confidential,
        [] as [&str; 0],
        [] as [&str; 0],
        2,
    );
    let now = 1_000_000_u64;

    // Fiel ao Python: a 1ª chamada da janela NÃO é registrada (histórico
    // vazio → permite sem append). Então com max=2: 1ª ok (não conta),
    // 2ª ok (registra 1), 3ª ok (registra 2), 4ª nega.
    assert!(engine.check_rate_limit_at("agente-a", now));
    assert!(engine.check_rate_limit_at("agente-a", now + 1));
    assert!(engine.check_rate_limit_at("agente-a", now + 2));
    assert!(!engine.check_rate_limit_at("agente-a", now + 3));

    // Limite é por agente: outro agente tem janela própria.
    assert!(engine.check_rate_limit_at("agente-b", now + 3));
}

#[test]
fn rate_limit_janela_deslizante_expira() {
    let engine = LocalPolicyEngine::new(
        SecurityLevel::Confidential,
        [] as [&str; 0],
        [] as [&str; 0],
        1,
    );
    let now = 1_000_000_u64;

    assert!(engine.check_rate_limit_at("a", now)); // 1ª não conta
    assert!(engine.check_rate_limit_at("a", now + 1)); // registra 1
    assert!(!engine.check_rate_limit_at("a", now + 2)); // limite atingido

    // 61 s depois a janela de 60 s expirou → permite de novo.
    assert!(engine.check_rate_limit_at("a", now + 61_000));
}

#[test]
fn rate_limit_prune_de_entradas_vazias() {
    let engine = LocalPolicyEngine::new(
        SecurityLevel::Confidential,
        [] as [&str; 0],
        [] as [&str; 0],
        1,
    );
    let now = 1_000_000_u64;

    assert!(engine.check_rate_limit_at("a", now)); // entrada criada e removida (vazia)
    assert_eq!(
        engine.tracked_agents(),
        0,
        "entrada vazia deve ser removida (anti memory-leak)"
    );

    assert!(engine.check_rate_limit_at("a", now + 1)); // agora registra
    assert_eq!(engine.tracked_agents(), 1);

    assert!(engine.check_rate_limit_at("a", now + 61_000)); // expirou → prune
    assert_eq!(engine.tracked_agents(), 0);
}

#[test]
fn identidade_do_agente_e_o_requester_id() {
    let r = req("t", 0, "request-123");
    assert_eq!(LocalPolicyEngine::agent_identity(&r), "request-123");
    assert_eq!(r.request_id, "correlation-id");
}

#[test]
fn evaluate_aplica_rate_limit() {
    let engine = LocalPolicyEngine::new(
        SecurityLevel::Confidential,
        [] as [&str; 0],
        [] as [&str; 0],
        1,
    );
    assert_eq!(
        engine.evaluate(&req("t", 0, "mesmo-agente")),
        ToolCallStatus::Allowed
    );
    assert_eq!(
        engine.evaluate(&req("t", 0, "mesmo-agente")),
        ToolCallStatus::Allowed
    );
    assert_eq!(
        engine.evaluate(&req("t", 0, "mesmo-agente")),
        ToolCallStatus::Denied
    );
}
