//! T-900-M3-01 — matriz de elegibilidade (V05) + janela de revogação.

#![cfg(not(feature = "dds"))]

use dds_contract::generated::dds_llm_orchestrator::AgentState;
use dds_contract::workflow::dds_llm_workflow_v1::AgentCapabilities;
use workflow_engine::eligibility::{is_eligible, validate_announcement, NotEligible};

const NOW: u64 = 1_000_000_000_000;

fn caps(labels: &[&str], valid_until: u64) -> AgentCapabilities {
    AgentCapabilities {
        agent_id: "agent-1".into(),
        schema_version: 1,
        announcement_revision: 1,
        capability_catalog_revision: 1,
        capabilities: labels.iter().map(|l| l.to_string()).collect(),
        model_ids: vec!["gemma-4-12b-it".into()],
        tool_versions: vec![],
        adaptive_protocol_version: 1,
        security_context_ref: "ctx".into(),
        created_at_ns: NOW - 1_000,
        valid_until_ns: valid_until,
    }
}

fn registry(health: i32, busy: u32, total: u32, model: &str) -> AgentState {
    AgentState {
        agent_id: "agent-1".into(),
        hostname: "h".into(),
        model: model.into(),
        specialization: "TEXT".into(),
        slots_total: total,
        slots_busy: busy,
        vram_total_mb: 1,
        vram_used_mb: 0,
        ema_latency_ms: 1.0,
        completed_total: 0,
        failed_total: 0,
        health,
        last_update_ns: NOW,
        uptime_seconds: 1,
    }
}

#[test]
fn eligible_when_all_conditions_hold() {
    let c = caps(&["rust_refactor", "unit_testing"], NOW + 5_000);
    let r = registry(2, 1, 4, "gemma-4-12b-it");
    assert!(is_eligible(
        Some(&c),
        Some(&r),
        &["rust_refactor".to_string()],
        Some("gemma-4-12b-it"),
        NOW
    )
    .is_ok());
}

#[test]
fn no_announcement_is_not_eligible() {
    assert_eq!(
        is_eligible(None, None, &[], None, NOW),
        Err(NotEligible::NoAnnouncement)
    );
}

#[test]
fn expired_announcement_is_not_eligible() {
    let c = caps(&["research"], NOW - 1);
    assert_eq!(
        validate_announcement(&c, NOW, &workflow_engine::catalog_v1()),
        Err(NotEligible::Expired {
            valid_until_ns: NOW - 1,
            now_ns: NOW
        })
    );
}

#[test]
fn unknown_labels_rejected() {
    let c = caps(&["magic"], NOW + 5_000);
    assert!(matches!(
        validate_announcement(&c, NOW, &workflow_engine::catalog_v1()),
        Err(NotEligible::UnknownLabels(_))
    ));
}

#[test]
fn protocol_and_catalog_version_pins() {
    let mut c = caps(&["research"], NOW + 5_000);
    c.adaptive_protocol_version = 2;
    assert!(matches!(
        validate_announcement(&c, NOW, &workflow_engine::catalog_v1()),
        Err(NotEligible::ProtocolVersion {
            got: 2,
            expected: 1
        })
    ));
    let mut c2 = caps(&["research"], NOW + 5_000);
    c2.capability_catalog_revision = 9;
    assert!(matches!(
        validate_announcement(&c2, NOW, &workflow_engine::catalog_v1()),
        Err(NotEligible::CatalogRevision {
            got: 9,
            expected: 1
        })
    ));
}

#[test]
fn missing_capability_is_not_eligible_v05() {
    // Agente TEXT com modelo certo, mas SEM security_audit: tarefa restrita
    // a security_audit NÃO é elegível (é o caso V05 do test-plan).
    let c = caps(&["rust_refactor"], NOW + 5_000);
    let r = registry(2, 0, 4, "gemma-4-12b-it");
    assert_eq!(
        is_eligible(
            Some(&c),
            Some(&r),
            &["security_audit".to_string()],
            None,
            NOW
        ),
        Err(NotEligible::MissingCapability("security_audit".into()))
    );
}

#[test]
fn registry_gates_health_slots_model() {
    let c = caps(&["rust_refactor"], NOW + 5_000);
    assert_eq!(
        is_eligible(Some(&c), Some(&registry(1, 0, 4, "m")), &[], None, NOW),
        Err(NotEligible::Unhealthy { health: 1 })
    );
    assert_eq!(
        is_eligible(Some(&c), Some(&registry(2, 4, 4, "m")), &[], None, NOW),
        Err(NotEligible::NoSlots { busy: 4, total: 4 })
    );
    assert_eq!(
        is_eligible(
            Some(&c),
            Some(&registry(2, 0, 4, "qwen")),
            &[],
            Some("gemma-4-12b-it"),
            NOW
        ),
        Err(NotEligible::ModelUnavailable("gemma-4-12b-it".into()))
    );
}

#[test]
fn revocation_window_revalidates_at_claim_time() {
    // Janela de revogação (M3-01): anúncio novo SEM a capacidade revoga a
    // elegibilidade IMEDIATAMENTE para novos claims — a checagem é no
    // momento do claim, não na admissão do workflow.
    let with_cap = caps(&["rust_refactor", "security_audit"], NOW + 10_000);
    let required = &["security_audit".to_string()];
    assert!(is_eligible(Some(&with_cap), None, required, None, NOW).is_ok());

    let mut revoked = caps(&["rust_refactor"], NOW + 10_000);
    revoked.announcement_revision = 2; // anúncio MAIS NOVO sem security_audit
    assert_eq!(
        is_eligible(Some(&revoked), None, required, None, NOW),
        Err(NotEligible::MissingCapability("security_audit".into()))
    );
}
