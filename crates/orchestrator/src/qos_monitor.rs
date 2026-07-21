//! Publicação de `QoS.Metric`/`QoS.Violation` — porte de
//! `src/orchestrator/dds_backend/qos_monitor.py` (`QoSMonitor`).
//!
//! **Escopo portado — e o que ficou de fora, deliberadamente:**
//! `qos_monitor.py` define 8 callbacks de listener nativo (`sample_lost`,
//! `sample_rejected`, `requested`/`offered_deadline_missed`, `liveliness_lost`,
//! `requested`/`offered_incompatible_qos`, `inconsistent_topic`) e 4
//! informativos (`data_available`, `subscription`/`publication_matched`,
//! `liveliness_changed`). Conferido em `orchestrator/main.py::make_data_space`:
//! **nenhum** desses callbacks é passado como listener a um reader/writer
//! real — só existem testados isoladamente com readers/status fake
//! (`tests/smoke_qos_*.py`, `tests/test_qos_monitor.py`). Em produção, o
//! `QoSMonitor` só roda de fato:
//! - `check_task_deadlines()` (polling) — portado aqui (ver `dds.rs`);
//! - `check_agent_liveliness()` (polling) — o Rust já tem detecção
//!   equivalente em `OrchestratorDds::reap_dead_agents` (via `last_seen`);
//!   em vez de duplicar o estado, o reaper passou a publicar a violação
//!   também, reaproveitando o `build_violation` deste módulo;
//! - `_publish_metrics()` — portado aqui.
//!
//! `check_reliability_gaps()` sempre retorna 0 em produção (lê um contador
//! que só um listener nunca-conectado incrementaria) — não portado.
//! `QoS.Discovery` fica sem produtor tanto no Python quanto no Rust (mesma
//! situação — não é uma regressão desta migração).
//!
//! Nota de `specs/CONTEXT.md`: "o listener nativo foi EVITADO no Python por
//! deadlock de GIL — em Rust o listener nativo é seguro e deve ser usado" é
//! uma intenção de design para trabalho futuro, não uma descrição do
//! comportamento atual do Python — por isso os 8 listeners ficam como
//! follow-up explícito (a crate `cyclonedds` já expõe todos eles via
//! `ListenerBuilder`), não uma "correção" silenciosa de escopo.

use dds_contract::generated::dds_llm_orchestrator::{QoSMetric, QoSViolation};

pub(crate) const COMPONENT: &str = "orchestrator";

/// Porte de `QoSMonitor.SEVERITY_MAP`.
pub(crate) fn severity(violation_type: &str) -> &'static str {
    match violation_type {
        "sample_rejected" | "sample_lost" | "liveliness_changed" => "WARNING",
        "requested_deadline_missed" | "offered_deadline_missed" => "ERROR",
        "liveliness_lost"
        | "requested_incompatible_qos"
        | "offered_incompatible_qos"
        | "inconsistent_topic" => "CRITICAL",
        _ => "WARNING",
    }
}

/// Monta um `QoSViolation` (porte de `_publish_violation`, sem side effects —
/// contadores e a chamada de escrita ficam a cargo do chamador em `dds.rs`).
pub(crate) fn build_violation(
    violation_type: &str,
    topic_name: &str,
    entity_kind: &str,
    affected_entity: &str,
    details: serde_json::Value,
    now_ns: u64,
) -> QoSViolation {
    QoSViolation {
        violation_id: format!("{violation_type}-{topic_name}-{now_ns}"),
        violation_type: violation_type.to_string(),
        topic_name: topic_name.to_string(),
        component: COMPONENT.to_string(),
        entity_kind: entity_kind.to_string(),
        affected_entity: affected_entity.to_string(),
        severity: severity(violation_type).to_string(),
        details_json: details.to_string(),
        timestamp_ns: now_ns,
        experiment_id: String::new(),
        run_id: String::new(),
        qos_profile: String::new(),
        load_level: String::new(),
        network_condition: String::new(),
        payload_level: String::new(),
        replication_idx: 0,
        warmup: false,
    }
}

/// Monta um `QoSMetric` (porte de um item do loop em `_publish_metrics`).
pub(crate) fn build_metric(
    metric_name: &str,
    value: i64,
    delta: i32,
    window_ms: i32,
    now_ns: u64,
) -> QoSMetric {
    QoSMetric {
        metric_id: format!("{COMPONENT}-{metric_name}"),
        metric_name: metric_name.to_string(),
        component: COMPONENT.to_string(),
        value,
        delta,
        window_ms,
        timestamp_ns: now_ns,
        experiment_id: String::new(),
        run_id: String::new(),
        qos_profile: String::new(),
        load_level: String::new(),
        network_condition: String::new(),
        payload_level: String::new(),
        replication_idx: 0,
        warmup: false,
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn severity_matches_python_severity_map() {
        assert_eq!(severity("requested_deadline_missed"), "ERROR");
        assert_eq!(severity("offered_deadline_missed"), "ERROR");
        assert_eq!(severity("liveliness_lost"), "CRITICAL");
        assert_eq!(severity("sample_lost"), "WARNING");
        assert_eq!(severity("nao-catalogado"), "WARNING");
    }

    #[test]
    fn violation_id_matches_python_format() {
        let v = build_violation(
            "requested_deadline_missed",
            "Tasks",
            "READER",
            "task-1",
            serde_json::json!({"task_id": "task-1"}),
            42,
        );
        assert_eq!(v.violation_id, "requested_deadline_missed-Tasks-42");
        assert_eq!(v.severity, "ERROR");
        assert_eq!(v.component, "orchestrator");
        assert_eq!(v.affected_entity, "task-1");
        assert_eq!(v.details_json, r#"{"task_id":"task-1"}"#);
    }

    #[test]
    fn metric_id_matches_python_format() {
        let m = build_metric("liveliness_lost", 3, 1, 5000, 7);
        assert_eq!(m.metric_id, "orchestrator-liveliness_lost");
        assert_eq!(m.value, 3);
        assert_eq!(m.delta, 1);
        assert_eq!(m.window_ms, 5000);
        assert_eq!(m.timestamp_ns, 7);
    }
}
