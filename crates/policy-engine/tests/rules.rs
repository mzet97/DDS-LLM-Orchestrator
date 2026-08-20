//! Testes das regras de `policies.json` — porte de `_check_policy`
//! (llm_gateway) e do bloco de cached policy (mcp_gateway), incluindo os
//! defaults quando a regra está ausente e a aplicação de deltas (extensão Rust).

use policy_engine::rules::{PolicyDecision, PolicyDocument};

/// O `policies.json` real (cópia idêntica do Python, version=2).
const POLICIES_JSON: &str = include_str!("../policies.json");

fn doc() -> PolicyDocument {
    PolicyDocument::from_json_str(POLICIES_JSON).expect("policies.json válido")
}

// ── llm_inference (porte de llm_gateway._check_policy) ─────────────────────

#[test]
fn llm_agente_nao_autorizado_nega() {
    assert!(matches!(
        doc().check_llm_request("EvilAgent", 0),
        PolicyDecision::Denied(_)
    ));
}

#[test]
fn llm_agente_autorizado_com_nivel_permitido() {
    assert_eq!(
        doc().check_llm_request("CodeReviewAgent", 0),
        PolicyDecision::Allowed
    );
    assert_eq!(
        doc().check_llm_request("CodeReviewAgent", 1),
        PolicyDecision::Allowed
    );
}

#[test]
fn llm_nivel_acima_do_permitido_nega() {
    // CodeReviewAgent: allowed_security_levels = [PUBLIC, INTERNAL].
    assert!(matches!(
        doc().check_llm_request("CodeReviewAgent", 2),
        PolicyDecision::Denied(_)
    ));
    // DocumentationAgent: só PUBLIC.
    assert!(matches!(
        doc().check_llm_request("DocumentationAgent", 1),
        PolicyDecision::Denied(_)
    ));
}

#[test]
fn llm_prefixo_de_role_herda_policy() {
    // Instâncias "<Agente>-<id>" herdam a policy do role (fallback por prefixo).
    assert_eq!(
        doc().check_llm_request("CodeReviewAgent-7", 1),
        PolicyDecision::Allowed
    );
    assert!(matches!(
        doc().check_llm_request("CodeReviewAgent-7", 2),
        PolicyDecision::Denied(_)
    ));
    // TestAgent só PUBLIC; a instância herda isso.
    assert_eq!(
        doc().check_llm_request("TestAgent-instancia", 0),
        PolicyDecision::Allowed
    );
    assert!(matches!(
        doc().check_llm_request("TestAgent-instancia", 1),
        PolicyDecision::Denied(_)
    ));
}

#[test]
fn llm_security_level_invalido_nega() {
    assert!(matches!(
        doc().check_llm_request("DocumentationAgent", -1),
        PolicyDecision::Denied(_)
    ));
    assert!(matches!(
        doc().check_llm_request("DocumentationAgent", 4),
        PolicyDecision::Denied(_)
    ));
}

#[test]
fn llm_sem_policy_configurada_allow_all() {
    let empty = PolicyDocument::empty();
    assert_eq!(
        empty.check_llm_request("QualquerAgente", 3),
        PolicyDecision::AllowedNoPolicy
    );
}

#[test]
fn llm_regra_presente_sem_entrada_do_agente_nega() {
    // `rules` existe mas `llm_inference` está ausente → allowed_agents=[]
    // → ninguém autorizado (equivale a default_action=DENY).
    let d = PolicyDocument::from_json_str(r#"{"version": 1, "rules": {}}"#).expect("doc");
    assert!(matches!(
        d.check_llm_request("CodeReviewAgent", 0),
        PolicyDecision::Denied(_)
    ));
}

// ── tool_call (porte do bloco cached policy do mcp_gateway) ───────────────

#[test]
fn tool_no_allowlist_permite() {
    assert_eq!(
        doc().check_tool_call("TestAgent", "filesystem.read_file"),
        PolicyDecision::Allowed
    );
    assert_eq!(
        doc().check_tool_call("CodeReviewAgent", "filesystem.list_directory"),
        PolicyDecision::Allowed
    );
}

#[test]
fn tool_fora_do_allowlist_nega() {
    // TestAgent só tem filesystem.read_file.
    assert!(matches!(
        doc().check_tool_call("TestAgent", "filesystem.list_directory"),
        PolicyDecision::Denied(_)
    ));
}

#[test]
fn tool_agente_sem_entrada_nega() {
    // `agent_allowlist.get(agent_id, [])` → [] → DENY (default_action=DENY).
    assert!(matches!(
        doc().check_tool_call("AgenteDesconhecido", "filesystem.read_file"),
        PolicyDecision::Denied(_)
    ));
}

#[test]
fn tool_high_risk_nega_mesmo_no_allowlist() {
    // No policies.json real nenhum allowlist contém filesystem.write_file,
    // então o teste usa um doc custom: allowlist contém, high_risk nega.
    let d = PolicyDocument::from_json_str(
        r#"{
            "version": 1,
            "rules": {
                "tool_call": {
                    "agent_tool_allowlist": {"A": ["filesystem.write_file"]},
                    "high_risk_tools": ["filesystem.write_file"],
                    "high_risk_action": "DENY",
                    "default_action": "DENY"
                }
            }
        }"#,
    )
    .expect("doc");
    let decision = d.check_tool_call("A", "filesystem.write_file");
    match decision {
        PolicyDecision::Denied(reason) => assert!(reason.contains("HIGH RISK")),
        other => panic!("esperava Denied, veio {other:?}"),
    }
}

#[test]
fn tool_sem_policy_configurada_allow_all() {
    let empty = PolicyDocument::empty();
    assert_eq!(
        empty.check_tool_call("QualquerAgente", "qualquer.tool"),
        PolicyDecision::AllowedNoPolicy
    );
}

// ── versão e ciclo de vida do documento ────────────────────────────────────

#[test]
fn version_do_documento() {
    assert_eq!(doc().version(), 2);
    assert_eq!(PolicyDocument::empty().version(), 0);
    assert_eq!(
        PolicyDocument::from_json_str(r#"{"rules": {}}"#)
            .expect("doc")
            .version(),
        0
    );
}

// ── apply_delta (extensão Rust — SecurityPolicyUpdate) ────────────────────

#[test]
fn delta_add_rule_faz_deep_merge() {
    let mut d = PolicyDocument::from_json_str(
        r#"{"version": 1, "rules": {"tool_call": {"high_risk_tools": ["a"], "default_action": "DENY"}}}"#,
    )
    .expect("doc");
    let delta = serde_json::json!({"rules": {"tool_call": {"high_risk_action": "DENY"}}});
    d.apply_delta("ADD_RULE", &delta).expect("delta válido");
    // Chave nova inserida, chaves existentes preservadas.
    let v = d.as_value();
    assert_eq!(v["rules"]["tool_call"]["high_risk_action"], "DENY");
    assert_eq!(
        v["rules"]["tool_call"]["high_risk_tools"],
        serde_json::json!(["a"])
    );
}

#[test]
fn delta_update_rule_substitui_valores() {
    let mut d = doc();
    let delta = serde_json::json!({"rules": {"llm_inference": {"allowed_agents": ["SóEle"]}}});
    d.apply_delta("UPDATE_RULE", &delta).expect("delta válido");
    // Array substituído (sem merge de arrays).
    assert!(matches!(
        d.check_llm_request("CodeReviewAgent", 0),
        PolicyDecision::Denied(_)
    ));
    assert!(matches!(
        d.check_llm_request("SóEle", 0),
        PolicyDecision::Denied(_)
    )); // sem agent_policy → DENY
}

#[test]
fn delta_remove_rule_remove_chaves_folha() {
    let mut d = doc();
    let delta = serde_json::json!({"rules": {"llm_inference": {"allowed_agents": true}}});
    d.apply_delta("REMOVE_RULE", &delta).expect("delta válido");
    // Sem allowed_agents → ninguém autorizado.
    assert!(matches!(
        d.check_llm_request("CodeReviewAgent", 0),
        PolicyDecision::Denied(_)
    ));
    // O resto do documento sobrevive.
    assert_eq!(d.version(), 2);
    assert!(d.as_value()["rules"]["tool_call"].is_object());
}

#[test]
fn delta_operacao_desconhecida_erro() {
    let mut d = doc();
    let delta = serde_json::json!({});
    assert!(d.apply_delta("DROP_TABLE", &delta).is_err());
}

#[test]
fn delta_nao_objeto_na_raiz_erro() {
    let mut d = doc();
    let delta = serde_json::json!(["não", "objeto"]);
    assert!(d.apply_delta("ADD_RULE", &delta).is_err());
}
