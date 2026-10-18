//! T-900-M5-03/04 — scheduler/materializer/recovery (V08/V09/V10).

#![cfg(not(feature = "dds"))]

use std::collections::HashSet;
use workflow_engine::dag::{CandidateNode, NodeKindCand, PlanCandidate};
use workflow_engine::engine::{EngineState, PlanGraph};
use workflow_engine::journal::Journal;

fn tmp(name: &str) -> std::path::PathBuf {
    let d = std::env::temp_dir().join(format!("wf-engine-test-{name}-{}", std::process::id()));
    let _ = std::fs::remove_dir_all(&d);
    d
}

fn node(id: &str, deps: &[&str]) -> CandidateNode {
    CandidateNode {
        node_id: id.into(),
        kind: NodeKindCand::Inference,
        required_capabilities: vec!["summarization".into()],
        model_id: String::new(),
        target_agent_id: String::new(),
        prompt_template: String::new(),
        dependencies: deps.iter().map(|s| s.to_string()).collect(),
        output_names: vec!["out".into()],
        inputs: vec![],
        max_attempts: 2,
        max_tokens: 10,
        max_output_bytes: 1024,
        deadline_ns: 0,
        priority: 5,
        side_effect_key: String::new(),
    }
}

fn fork_join() -> PlanGraph {
    let c = PlanCandidate {
        planner_id: "static-template".into(),
        planner_revision: "v".into(),
        planner_tokens: 0,
        nodes: vec![
            node("a", &[]),
            node("b1", &["a"]),
            node("b2", &["a"]),
            node("join", &["b1", "b2"]),
        ],
    };
    PlanGraph::from_candidate("wf-x", 1, &c)
}

#[test]
fn join_waits_for_all_deps_regardless_of_arrival_order() {
    let dir = tmp("join");
    let j = Journal::open(&dir).unwrap();
    j.admit_workflow(
        "wf-x", "req", "dec", "h", "accept", 1_000, 1_000, "prompt", 1, 9, 1, 1, 1, 1, "",
    )
    .unwrap();
    let mut st = EngineState::default();
    let g = fork_join();
    st.materialize(&j, &g, "a", 0).unwrap();
    // a DONE → b1 e b2 prontos
    let ready = st
        .on_done(&j, &g, "a", 1, &[("out".into(), "h".into())])
        .unwrap();
    assert_eq!(ready, vec!["b1", "b2"]);
    st.materialize(&j, &g, "b1", 0).unwrap();
    st.materialize(&j, &g, "b2", 0).unwrap();
    // b2 chega PRIMEIRO (fora de ordem): join NÃO libera (só b1 segue pendente)
    let ready2 = st
        .on_done(&j, &g, "b2", 1, &[("out".into(), "h".into())])
        .unwrap();
    assert!(
        !ready2.contains(&"join".to_string()),
        "join só dispara com TODAS as dependências"
    );
    // b1 chega depois: join libera (dependência, não ordem)
    let ready3 = st
        .on_done(&j, &g, "b1", 1, &[("out".into(), "h".into())])
        .unwrap();
    assert_eq!(ready3, vec!["join"]);
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn duplicate_done_is_ignored_typed() {
    let dir = tmp("dup");
    let j = Journal::open(&dir).unwrap();
    j.admit_workflow(
        "wf-x", "req", "dec", "h", "accept", 1_000, 1_000, "prompt", 1, 9, 1, 1, 1, 1, "",
    )
    .unwrap();
    let mut st = EngineState::default();
    let g = fork_join();
    st.materialize(&j, &g, "a", 0).unwrap();
    st.on_done(&j, &g, "a", 1, &[("out".into(), "h".into())])
        .unwrap();
    // DUPLICATA da mesma conclusão: erro tipado, sem efeitos novos
    assert!(matches!(
        st.on_done(&j, &g, "a", 1, &[("out".into(), "h".into())]),
        Err(workflow_engine::engine::EngineError::AlreadyDone(_))
    ));
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn invalid_output_rejects_done() {
    let dir = tmp("badout");
    let j = Journal::open(&dir).unwrap();
    j.admit_workflow(
        "wf-x", "req", "dec", "h", "accept", 1_000, 1_000, "prompt", 1, 9, 1, 1, 1, 1, "",
    )
    .unwrap();
    let mut st = EngineState::default();
    let g = fork_join();
    st.materialize(&j, &g, "a", 0).unwrap();
    // output declarado ausente/hash vazio → DONE rejeitado, nó segue ativo
    assert!(matches!(
        st.on_done(&j, &g, "a", 1, &[("out".into(), String::new())]),
        Err(workflow_engine::engine::EngineError::InvalidOutput(_))
    ));
    let ready = st
        .on_done(&j, &g, "a", 1, &[("out".into(), "h".into())])
        .unwrap();
    assert_eq!(ready.len(), 2);
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn stale_attempt_output_does_not_conclude_new_attempt() {
    let dir = tmp("stale");
    let j = Journal::open(&dir).unwrap();
    j.admit_workflow(
        "wf-x", "req", "dec", "h", "accept", 1_000, 1_000, "prompt", 1, 9, 1, 1, 1, 1, "",
    )
    .unwrap();
    let mut st = EngineState::default();
    let g = fork_join();
    st.materialize(&j, &g, "a", 0).unwrap();
    // falha com failover → tentativa 2 ativa
    let next = st.on_failed(&j, &g, "a", 1, true).unwrap();
    assert_eq!(next, Some(2));
    st.materialize(&j, &g, "a", 0).unwrap();
    // SAÍDA TARDIA da tentativa 1: ignorada (não conclui a 2)
    assert!(matches!(
        st.on_done(&j, &g, "a", 1, &[("out".into(), "h".into())]),
        Err(workflow_engine::engine::EngineError::StaleAttempt {
            attempt: 1,
            active: 2,
            ..
        })
    ));
    // a tentativa 2 conclui normalmente
    let ready = st
        .on_done(&j, &g, "a", 2, &[("out".into(), "h".into())])
        .unwrap();
    assert_eq!(ready.len(), 2);
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn retry_creates_new_task_id_and_reservation_is_not_renewed() {
    let dir = tmp("retry");
    let j = Journal::open(&dir).unwrap();
    j.admit_workflow(
        "wf-x", "req", "dec", "h", "accept", 1_000, 1_000, "prompt", 1, 9, 1, 1, 1, 1, "",
    )
    .unwrap();
    let mut st = EngineState::default();
    let g = fork_join();
    let t1 = st.materialize(&j, &g, "a", 5).unwrap();
    st.on_failed(&j, &g, "a", 1, true).unwrap();
    let t2 = st.materialize(&j, &g, "a", 5).unwrap();
    assert_ne!(t1, t2, "tentativa nova = task_id físico novo");
    // orçamento: tentativa 1 foi cobrada INTEGRAL (consumo incerto) e a 2
    // reserva de novo — comprometido >= 10, sem renovação da mesma reserva.
    let (_, reserved, consumed) = j.budget_status("wf-x").unwrap();
    assert!(reserved + consumed >= 10);
    // replay do materialize da MESMA tentativa: idempotente
    let t2b = st.materialize(&j, &g, "a", 5).unwrap();
    assert_eq!(t2, t2b);
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn without_failover_first_failure_fails_node() {
    let dir = tmp("nofailover");
    let j = Journal::open(&dir).unwrap();
    j.admit_workflow(
        "wf-x", "req", "dec", "h", "accept", 1_000, 1_000, "prompt", 1, 9, 1, 1, 1, 1, "",
    )
    .unwrap();
    let mut st = EngineState::default();
    let g = fork_join();
    st.materialize(&j, &g, "a", 5).unwrap();
    assert_eq!(st.on_failed(&j, &g, "a", 1, false).unwrap(), None);
    assert!(st.nodes["a"].failed);
    drop(j);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recovery_after_restart_resumes_same_active_attempt() {
    let dir = tmp("recover");
    let g = fork_join();
    let t_attempt1;
    {
        let j = Journal::open(&dir).unwrap();
        j.admit_workflow(
            "wf-x", "req", "dec", "h", "accept", 1_000, 1_000, "prompt", 1, 9, 1, 1, 1, 1, "",
        )
        .unwrap();
        let mut st = EngineState::default();
        t_attempt1 = st.materialize(&j, &g, "a", 0).unwrap();
        // crash ANTES do done: tentativa 1 publicada, não concluída
    }
    let j2 = Journal::open(&dir).unwrap();
    let mut st2 = EngineState::recover(&j2, &g).unwrap();
    assert!(!st2.nodes["a"].done, "restart não inventa conclusão");
    assert_eq!(st2.nodes["a"].active_attempt, 1, "retoma a MESMA tentativa");
    // re-materializa idempotente: mesmo task_id físico
    let t_again = st2.materialize(&j2, &g, "a", 0).unwrap();
    assert_eq!(t_attempt1, t_again);
    drop(j2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn recovery_keeps_done_set_and_unlocks_dependents() {
    let dir = tmp("recover-done");
    let g = fork_join();
    {
        let j = Journal::open(&dir).unwrap();
        j.admit_workflow(
            "wf-x", "req", "dec", "h", "accept", 1_000, 1_000, "prompt", 1, 9, 1, 1, 1, 1, "",
        )
        .unwrap();
        let mut st = EngineState::default();
        st.materialize(&j, &g, "a", 0).unwrap();
        st.on_done(&j, &g, "a", 1, &[("out".into(), "h".into())])
            .unwrap();
    }
    let j2 = Journal::open(&dir).unwrap();
    let st2 = EngineState::recover(&j2, &g).unwrap();
    let done: HashSet<String> = st2.done_set();
    assert!(done.contains("a"));
    let ready: Vec<&str> = g
        .ready_nodes(&done)
        .iter()
        .map(|n| n.node_id.as_str())
        .collect();
    assert!(ready.contains(&"b1") && ready.contains(&"b2"));
    drop(j2);
    let _ = std::fs::remove_dir_all(&dir);
}

#[test]
fn direct_template_planner_produces_single_node_without_llm() {
    use workflow_engine::engine::{DirectTemplatePlanner, Planner};
    let p = DirectTemplatePlanner
        .plan(&workflow_engine::engine::PlanRequest {
            workflow_id: "wf".into(),
            prompt: "resuma".into(),
        })
        .unwrap();
    assert_eq!(p.planner_id, "static-template");
    assert_eq!(p.nodes.len(), 1); // tarefa simples NÃO chama planner LLM
}
