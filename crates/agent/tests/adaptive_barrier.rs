//! T-900-M3-02/03 — barreira adaptativa: V05 (elegibilidade), V06 (ordem de
//! chegada Plan/State/Task), target exato, tentativa ativa, revogação na
//! janela e contenção K=2/K=4 (in-memory; fixtures — o motor completo é M5).
//!
//! Gate `not(dds)` (mocks); o caminho DDS real da barreira é exercitado pelo
//! modo `--adaptive` do bin (feeders + anúncio) no harness da fase.

#![cfg(not(feature = "dds"))]

use agent::adaptive::{barrier_check, laya1_task_id, AdaptiveDecision, AdaptiveSnapshot, Blocked};
use dds_contract::generated::dds_llm_orchestrator::{AgentState, Task};
use dds_contract::workflow::dds_llm_workflow_v1::{
    AgentCapabilities, NodeState, PlanNode, WorkflowPlan, WorkflowState,
};

const NOW: u64 = 1_000_000_000_000;
const AGENT: &str = "agent-a";

fn node(id: &str, caps: &[&str], target: &str) -> PlanNode {
    PlanNode {
        node_id: id.into(),
        kind: 0,
        required_capabilities: caps.iter().map(|c| c.to_string()).collect(),
        model_id: String::new(),
        target_agent_id: target.into(),
        prompt_template: String::new(),
        dependencies: vec![],
        inputs: vec![],
        output_names: vec![],
        max_attempts: 2,
        deadline_ns: NOW + 1_000_000,
        max_tokens: 10,
        max_output_bytes: 10,
        security_context_ref: String::new(),
        priority: 5,
        side_effect_key: String::new(),
    }
}

fn plan(wf: &str, nodes: Vec<PlanNode>) -> WorkflowPlan {
    WorkflowPlan {
        workflow_id: wf.into(),
        plan_revision: 1,
        schema_version: 1,
        request_id: "req".into(),
        intent_revision: 1,
        effective_decision_id: "dec".into(),
        policy_revision: 1,
        capability_catalog_revision: 1,
        planner_id: "static-template".into(),
        planner_revision: "r".into(),
        created_at_ns: NOW - 10,
        deadline_ns: NOW + 10_000_000,
        qos_profile: 0,
        nodes,
        plan_hash: "h".into(),
    }
}

fn state(wf: &str, nodes: Vec<NodeState>) -> WorkflowState {
    WorkflowState {
        workflow_id: wf.into(),
        plan_revision: 1,
        schema_version: 1,
        state_revision: 1,
        request_id: "req".into(),
        effective_decision_id: "dec".into(),
        plan_hash: "h".into(),
        status: 3, // RUNNING
        reason_code: 0,
        nodes,
        created_at_ns: NOW - 5,
        updated_at_ns: NOW - 1,
        expires_at_ns: NOW + 10_000_000,
        cancelled_by_request_id: String::new(),
        effective_qos_config_hash: "q".into(),
        fallback_template_id: String::new(),
    }
}

fn node_state(id: &str, status: i32, attempt: u32) -> NodeState {
    NodeState {
        node_id: id.into(),
        status,
        attempt_id: attempt,
        task_id: String::new(),
        assigned_agent_id: String::new(),
        ready: true,
        started_at_ns: 0,
        completed_at_ns: 0,
        outputs: vec![],
        reason_code: 0,
    }
}

fn announcement(agent: &str, caps: &[&str], revision: u32) -> AgentCapabilities {
    AgentCapabilities {
        agent_id: agent.into(),
        schema_version: 1,
        announcement_revision: revision,
        capability_catalog_revision: 1,
        capabilities: caps.iter().map(|c| c.to_string()).collect(),
        model_ids: vec![],
        tool_versions: vec![],
        adaptive_protocol_version: 1,
        security_context_ref: String::new(),
        created_at_ns: NOW - 100,
        valid_until_ns: NOW + 10_000,
    }
}

fn registry(busy: u32) -> AgentState {
    AgentState {
        agent_id: AGENT.into(),
        hostname: "h".into(),
        model: String::new(),
        specialization: "TEXT".into(),
        slots_total: 4,
        slots_busy: busy,
        vram_total_mb: 1,
        vram_used_mb: 0,
        ema_latency_ms: 1.0,
        completed_total: 0,
        failed_total: 0,
        health: 2,
        last_update_ns: NOW,
        uptime_seconds: 1,
    }
}

fn laya1_task_for(p: &WorkflowPlan, node_id: &str, attempt: u32, target: &str) -> Task {
    let tid = laya1_task_id(&p.workflow_id, p.plan_revision, node_id, attempt);
    Task {
        task_id: tid,
        client_id: "engine".into(),
        assigned_agent: String::new(),
        target_agent: target.into(),
        model_required: 0,
        model_name: String::new(),
        messages_json: "[]".into(),
        temperature: 0.1,
        max_tokens: 10,
        stream: false,
        status: 0,
        priority: 5,
        created_at_ns: NOW,
        assigned_at_ns: 0,
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns: NOW + 1_000_000,
        retry_count: 0,
        finish_reason: String::new(),
        t_serialization_ns: 0,
        t_transport_send_ns: 0,
        t_agent_queue_ns: 0,
        t_inference_ns: 0,
        t_transport_return_ns: 0,
        t_deserialization_ns: 0,
    }
}

fn full_snapshot() -> (AdaptiveSnapshot, WorkflowPlan) {
    let p = plan("wf-1", vec![node("node-a", &["security_audit"], "")]);
    let mut snap = AdaptiveSnapshot::default();
    snap.upsert_plan(p.clone());
    snap.upsert_state(state(
        "wf-1",
        vec![node_state("node-a", 2, 1)], // PUBLISHED, tentativa 1
    ));
    snap.upsert_capabilities(announcement(AGENT, &["security_audit", "rust_refactor"], 1));
    (snap, p)
}

#[test]
fn v05_proceeds_when_capable() {
    let (snap, p) = full_snapshot();
    let task = laya1_task_for(&p, "node-a", 1, "");
    assert_eq!(
        barrier_check(
            &snap,
            &task,
            AGENT,
            snap.capabilities.get(AGENT),
            Some(&registry(0)),
            "",
            NOW
        ),
        AdaptiveDecision::Proceed
    );
}

#[test]
fn v05_blocks_without_capability() {
    let (snap, p) = full_snapshot();
    let mut snap = snap;
    snap.upsert_capabilities(announcement(AGENT, &["rust_refactor"], 1)); // SEM security_audit
    let task = laya1_task_for(&p, "node-a", 1, "");
    match barrier_check(
        &snap,
        &task,
        AGENT,
        snap.capabilities.get(AGENT),
        Some(&registry(0)),
        "",
        NOW,
    ) {
        AdaptiveDecision::Block(Blocked::NotEligible(_)) => {}
        other => panic!("esperava Block(NotEligible), veio {other:?}"),
    }
}

#[test]
fn v06_task_before_plan_is_blocked_until_barrier_completes() {
    let (full, p) = full_snapshot();
    let task = laya1_task_for(&p, "node-a", 1, "");

    // Permutação 1: SEM plan → bloqueada
    let mut snap = AdaptiveSnapshot::default();
    snap.upsert_state(state("wf-1", vec![node_state("node-a", 2, 1)]));
    snap.upsert_capabilities(announcement(AGENT, &["security_audit"], 1));
    assert_eq!(
        barrier_check(
            &snap,
            &task,
            AGENT,
            snap.capabilities.get(AGENT),
            None,
            "",
            NOW
        ),
        AdaptiveDecision::Block(Blocked::UnresolvedNode)
    );

    // Permutação 2: SEM state → bloqueada
    let mut snap2 = AdaptiveSnapshot::default();
    snap2.upsert_plan(p.clone());
    snap2.upsert_capabilities(announcement(AGENT, &["security_audit"], 1));
    assert_eq!(
        barrier_check(
            &snap2,
            &task,
            AGENT,
            snap2.capabilities.get(AGENT),
            None,
            "",
            NOW
        ),
        AdaptiveDecision::Block(Blocked::UnresolvedNode)
    );

    // Completando a barreira (qualquer ordem posterior): prossegue
    assert_eq!(
        barrier_check(
            &full,
            &task,
            AGENT,
            full.capabilities.get(AGENT),
            None,
            "",
            NOW
        ),
        AdaptiveDecision::Proceed
    );
}

#[test]
fn target_requires_exact_match_even_with_empty_prefix_config() {
    // Achado A04: prefixo vazio do legado NÃO autoriza instância errada no
    // modo adaptativo.
    let (snap, p) = full_snapshot();
    let task = laya1_task_for(&p, "node-a", 1, "agent-OUTRO");
    assert_eq!(
        barrier_check(
            &snap,
            &task,
            AGENT,
            snap.capabilities.get(AGENT),
            None,
            "",
            NOW
        ),
        AdaptiveDecision::Block(Blocked::TargetMismatch {
            task_target: "agent-OUTRO".into(),
            agent_id: AGENT.into(),
        })
    );
    // Target no NÓ reservado a outro agente também bloqueia
    let mut snap2 = snap;
    let p2 = plan("wf-2", vec![node("node-b", &["security_audit"], "agent-b")]);
    snap2.upsert_plan(p2.clone());
    snap2.upsert_state(state("wf-2", vec![node_state("node-b", 2, 1)]));
    let task2 = laya1_task_for(&p2, "node-b", 1, "");
    assert!(matches!(
        barrier_check(
            &snap2,
            &task2,
            AGENT,
            snap2.capabilities.get(AGENT),
            None,
            "",
            NOW
        ),
        AdaptiveDecision::Block(Blocked::NodeTargeted(_))
    ));
}

#[test]
fn stale_attempt_task_id_does_not_resolve() {
    // Saída/tentativa antiga: task_id da tentativa 1 não casa quando a ativa
    // é a 2 (retomada incremental por attempt).
    let (snap, p) = full_snapshot();
    let mut snap = snap;
    snap.upsert_state(state("wf-1", vec![node_state("node-a", 2, 2)])); // ativa = 2
    let old = laya1_task_for(&p, "node-a", 1, "");
    assert_eq!(
        barrier_check(
            &snap,
            &old,
            AGENT,
            snap.capabilities.get(AGENT),
            None,
            "",
            NOW
        ),
        AdaptiveDecision::Block(Blocked::UnresolvedNode)
    );
    let current = laya1_task_for(&p, "node-a", 2, "");
    assert_eq!(
        barrier_check(
            &snap,
            &current,
            AGENT,
            snap.capabilities.get(AGENT),
            None,
            "",
            NOW
        ),
        AdaptiveDecision::Proceed
    );
}

#[test]
fn non_claimable_node_status_blocks() {
    let (snap, p) = full_snapshot();
    let mut snap = snap;
    snap.upsert_state(state("wf-1", vec![node_state("node-a", 5, 1)])); // DONE
    let task = laya1_task_for(&p, "node-a", 1, "");
    assert!(matches!(
        barrier_check(
            &snap,
            &task,
            AGENT,
            snap.capabilities.get(AGENT),
            None,
            "",
            NOW
        ),
        AdaptiveDecision::Block(Blocked::NodeNotClaimable { .. })
    ));
}

#[test]
fn legacy_tasks_untouched_by_barrier() {
    let snap = AdaptiveSnapshot::default(); // VAZIA: sem plan/state
    let mut task = laya1_task_for(&plan("wf-x", vec![]), "n", 1, "");
    task.task_id = "task-comum-123".into();
    assert_eq!(
        barrier_check(&snap, &task, AGENT, None, None, "", NOW),
        AdaptiveDecision::Legacy
    );
}

#[test]
fn revocation_between_pre_and_post_check_blocks() {
    // Janela de revogação: o mesmo task_id era Proceed; um anúncio NOVO sem
    // a capacidade chega entre as checagens → a segunda (pós-readback) bloqueia.
    let (snap, p) = full_snapshot();
    let task = laya1_task_for(&p, "node-a", 1, "");
    assert_eq!(
        barrier_check(
            &snap,
            &task,
            AGENT,
            snap.capabilities.get(AGENT),
            Some(&registry(0)),
            "",
            NOW
        ),
        AdaptiveDecision::Proceed
    );
    let mut revoked = snap;
    revoked.upsert_capabilities(announcement(AGENT, &["rust_refactor"], 2));
    assert!(matches!(
        barrier_check(
            &revoked,
            &task,
            AGENT,
            revoked.capabilities.get(AGENT),
            Some(&registry(0)),
            "",
            NOW
        ),
        AdaptiveDecision::Block(Blocked::NotEligible(_))
    ));
}

#[test]
fn contention_k2_only_capable_agent_proceeds() {
    // K=2: agentes A (com security_audit) e B (sem) disputam a mesma task;
    // apenas A passa pela barreira — B nem tenta o claim.
    let (snap, p) = full_snapshot();
    let task = laya1_task_for(&p, "node-a", 1, "");
    let mut snap_b = snap.clone();
    snap_b.upsert_capabilities(announcement("agent-b", &["summarization"], 1));
    let a = barrier_check(
        &snap,
        &task,
        AGENT,
        snap.capabilities.get(AGENT),
        Some(&registry(0)),
        "",
        NOW,
    );
    let b = barrier_check(
        &snap_b,
        &task,
        "agent-b",
        snap_b.capabilities.get("agent-b"),
        Some(&registry(0)),
        "",
        NOW,
    );
    assert_eq!(a, AdaptiveDecision::Proceed);
    assert!(matches!(
        b,
        AdaptiveDecision::Block(Blocked::NotEligible(_))
    ));
}

#[test]
fn contention_k4_single_attempt_single_winner_by_attempt_id() {
    // K=4: quatro agentes capazes; o task_id contém a TENTATIVA: todos
    // passam a barreira (o arbitration real é o protocolo de claim legado —
    // Exclusive Ownership + readback); a barreira garante que disputam a
    // MESMA tentativa ativa e que instâncias sem slots ficam fora.
    let (snap, p) = full_snapshot();
    let task = laya1_task_for(&p, "node-a", 1, "");
    for name in ["a1", "a2", "a3", "a4"] {
        let mut s = snap.clone();
        s.upsert_capabilities(announcement(name, &["security_audit"], 1));
        assert_eq!(
            barrier_check(
                &s,
                &task,
                name,
                s.capabilities.get(name),
                Some(&registry(0)),
                "",
                NOW
            ),
            AdaptiveDecision::Proceed,
            "{name} deveria disputar"
        );
    }
    // Agente sem slot: fora mesmo sendo capaz.
    let mut s = snap.clone();
    s.upsert_capabilities(announcement("full", &["security_audit"], 1));
    assert!(matches!(
        barrier_check(
            &s,
            &task,
            "full",
            s.capabilities.get("full"),
            Some(&registry(4)),
            "",
            NOW
        ),
        AdaptiveDecision::Block(Blocked::NotEligible(_))
    ));
}

#[test]
fn restart_recovers_barrier_from_snapshots() {
    // Restart/failover (M3-03): a barreira é estado DERIVADO dos tópicos —
    // reconstituída a partir dos snapshots (TransientLocal), sem estado local.
    let (snap, p) = full_snapshot();
    let task = laya1_task_for(&p, "node-a", 1, "");
    let mut rebuilt = AdaptiveSnapshot::default();
    // "Replay" fora de ordem (estado antes do plano): resultado idêntico.
    rebuilt.upsert_state(snap.states["wf-1"].clone());
    rebuilt.upsert_capabilities(announcement(AGENT, &["security_audit"], 1));
    rebuilt.upsert_plan(snap.plans["wf-1"].clone());
    assert_eq!(
        barrier_check(
            &rebuilt,
            &task,
            AGENT,
            rebuilt.capabilities.get(AGENT),
            None,
            "",
            NOW
        ),
        AdaptiveDecision::Proceed
    );
}
