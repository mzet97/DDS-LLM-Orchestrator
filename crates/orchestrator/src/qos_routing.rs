//! Publicação de `QoS.RoutingProfile` — porte de
//! `src/orchestrator/orchestrator/fuzzy_routing_policy.py` +
//! `Orchestrator._publish_fuzzy_routing_profile` (main.py).
//!
//! Opt-in via `--fuzzy-routing` (default OFF, como `enable_fuzzy_routing` no
//! Python): quando ligado, cada decisão de QoS que troca de perfil publica um
//! `QoSRoutingProfile` recomendando pesos de roteamento por prefixo de agente
//! — consumido por um selector de agente ponderado (fora do escopo desta
//! migração; o Python também só declarava o tópico).
//!
//! **Preservado do Python, incluindo detalhes não-óbvios:**
//! - `preferred_agent_prefix` é sempre `""` para os 5 perfis conhecidos — o
//!   comentário original explica: um preferido "roubaria" tasks já reivindicadas
//!   por outro agente no modo data-centric; o fallback fica alto (300s) para
//!   que só o agente sorteado pelos pesos assuma a task.
//! - `allowed_agent_prefixes_json` é uma LISTA — `json.dumps(lista, sort_keys=True)`
//!   no Python NÃO ordena listas (`sort_keys` só afeta dicts); a ordem é a de
//!   inserção do dict `weights` de cada perfil. `QoS_StreamLike` insere RTX
//!   antes de AMD — por isso preservamos a ordem por perfil explicitamente
//!   abaixo em vez de reconstruí-la a partir de `weights` (que é ordenado).
//! - `weights_json`/`explanation_json` usam dicts → `sort_keys=True` ordena
//!   alfabeticamente; `serde_json::Map` (sem a feature `preserve_order`, que o
//!   workspace não liga) já serializa em ordem alfabética por padrão.
//! - Dedup: só publica quando `profile_name` muda desde a última publicação
//!   (`_last_routing_profile_name`); a versão só incrementa nesse caso.

use dds_contract::generated::dds_llm_orchestrator::QoSRoutingProfile;

/// Prefixo do agente AMD (RX 7900 XTX) — `DDS_AGENT_AMD_PREFIX` no Python.
fn amd_prefix() -> String {
    std::env::var("DDS_AGENT_AMD_PREFIX").unwrap_or_else(|_| "agent-rx7900xtx-gemma4".to_string())
}

/// Prefixo do agente RTX (RTX 3080) — `DDS_AGENT_RTX_PREFIX` no Python.
fn rtx_prefix() -> String {
    std::env::var("DDS_AGENT_RTX_PREFIX").unwrap_or_else(|_| "agent-rtx3080-gemma4".to_string())
}

/// Entrada de roteamento de um perfil: pesos por prefixo de agente **na ordem
/// de inserção do Python** (ver nota do módulo sobre `allowed_agent_prefixes_json`).
struct RoutingEntry {
    preferred: &'static str,
    /// `(prefixo, peso)` na ordem de inserção do `PROFILE_ROUTING` do Python.
    weights: Vec<(String, f64)>,
    fallback_after_ms: i32,
}

/// Porte de `PROFILE_ROUTING`/`map_fuzzy_profile` (`fuzzy_routing_policy.py`).
/// Nome desconhecido cai no default `QoS_Balanced` (paridade com `dict.get`).
fn map_fuzzy_profile(profile_name: &str) -> RoutingEntry {
    let amd = amd_prefix();
    let rtx = rtx_prefix();
    match profile_name {
        "QoS_LowCost" => RoutingEntry {
            preferred: "",
            weights: vec![(amd, 0.75), (rtx, 0.25)],
            fallback_after_ms: 300_000,
        },
        "QoS_Critical" => RoutingEntry {
            preferred: "",
            weights: vec![(amd, 0.25), (rtx, 0.75)],
            fallback_after_ms: 300_000,
        },
        "QoS_StreamLike" => RoutingEntry {
            preferred: "",
            // RTX primeiro — ver nota do módulo (ordem de inserção do Python).
            weights: vec![(rtx, 0.60), (amd, 0.40)],
            fallback_after_ms: 300_000,
        },
        "QoS_Failover" => RoutingEntry {
            preferred: "",
            weights: vec![(amd, 0.50), (rtx, 0.50)],
            fallback_after_ms: 300_000,
        },
        // "QoS_Balanced" e qualquer nome desconhecido (DEFAULT_ROUTING no Python).
        _ => RoutingEntry {
            preferred: "",
            weights: vec![(amd, 0.50), (rtx, 0.50)],
            fallback_after_ms: 300_000,
        },
    }
}

/// Monta o `QoSRoutingProfile` para publicação (porte de
/// `_publish_fuzzy_routing_profile`, sem o dedup/versionamento — isso fica no
/// chamador, que tem o estado do `OrchestratorDds`).
///
/// `profile_id` fica sempre `"GLOBAL"` (default do dataclass Python, nunca
/// sobrescrito no `_publish_fuzzy_routing_profile`).
pub(crate) fn build_routing_profile(
    profile_name: &str,
    version: i32,
    centroid: f64,
    now_ns: u64,
) -> QoSRoutingProfile {
    let routing = map_fuzzy_profile(profile_name);

    let allowed_agent_prefixes_json = serde_json::to_string(
        &routing
            .weights
            .iter()
            .map(|(prefix, _)| prefix.clone())
            .collect::<Vec<_>>(),
    )
    .unwrap_or_else(|_| "[]".to_string());

    let weights_map: serde_json::Map<String, serde_json::Value> = routing
        .weights
        .iter()
        .map(|(prefix, w)| (prefix.clone(), serde_json::json!(w)))
        .collect();
    let weights_json = serde_json::to_string(&weights_map).unwrap_or_else(|_| "{}".to_string());

    let explanation_json = serde_json::json!({
        "source": "fuzzy_qos",
        "profile": profile_name,
    })
    .to_string();

    QoSRoutingProfile {
        profile_id: "GLOBAL".to_string(),
        version,
        profile_name: profile_name.to_string(),
        preferred_agent_prefix: routing.preferred.to_string(),
        allowed_agent_prefixes_json,
        weights_json,
        fallback_after_ms: routing.fallback_after_ms,
        centroid_score: centroid as f32,
        explanation_json,
        timestamp_ns: now_ns,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn balanced_and_unknown_profile_share_default_routing() {
        let balanced = build_routing_profile("QoS_Balanced", 1, 0.5, 1);
        let unknown = build_routing_profile("nao-existe", 1, 0.5, 1);
        assert_eq!(balanced.weights_json, unknown.weights_json);
        assert_eq!(balanced.fallback_after_ms, 300_000);
        assert_eq!(balanced.preferred_agent_prefix, "");
    }

    #[test]
    fn stream_like_orders_rtx_before_amd_in_allowed_prefixes() {
        let p = build_routing_profile("QoS_StreamLike", 3, 0.9, 42);
        let allowed: Vec<String> = serde_json::from_str(&p.allowed_agent_prefixes_json).unwrap();
        assert_eq!(allowed[0], rtx_prefix());
        assert_eq!(allowed[1], amd_prefix());
    }

    #[test]
    fn weights_json_keys_are_alphabetically_sorted() {
        // amd_prefix() = "agent-rx7900xtx-gemma4", rtx_prefix() = "agent-rtx3080-gemma4"
        // "agent-rt..." < "agent-rx..." alfabeticamente.
        let p = build_routing_profile("QoS_LowCost", 1, 0.0, 0);
        let v: serde_json::Value = serde_json::from_str(&p.weights_json).unwrap();
        let keys: Vec<&String> = v.as_object().unwrap().keys().collect();
        let mut sorted = keys.clone();
        sorted.sort();
        assert_eq!(
            keys, sorted,
            "weights_json deve ter chaves em ordem alfabética (paridade com sort_keys=True)"
        );
    }

    #[test]
    fn explanation_json_carries_profile_and_source() {
        let p = build_routing_profile("QoS_Failover", 5, 0.3, 7);
        let v: serde_json::Value = serde_json::from_str(&p.explanation_json).unwrap();
        assert_eq!(v["source"], "fuzzy_qos");
        assert_eq!(v["profile"], "QoS_Failover");
    }

    #[test]
    fn profile_id_is_always_global() {
        let p = build_routing_profile("QoS_Critical", 1, 1.0, 1);
        assert_eq!(p.profile_id, "GLOBAL");
    }

    #[test]
    fn centroid_score_casts_confidence_to_f32() {
        let p = build_routing_profile("QoS_Balanced", 1, 0.82, 1);
        assert!((p.centroid_score - 0.82_f32).abs() < 1e-6);
    }
}
