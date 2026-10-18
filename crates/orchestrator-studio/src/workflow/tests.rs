use super::*;
use std::time::Duration;

/// O poll roda por frame na GUI; nos testes, drena até o fim com teto
/// de espera (a thread do worker é concorrente de verdade).
fn poll_until_idle(state: &mut WorkflowState) {
    for _ in 0..400 {
        state.poll();
        if !state.busy {
            return;
        }
        thread::sleep(Duration::from_millis(5));
    }
    panic!("worker não concluiu em 2s");
}

#[test]
fn start_transitions_and_worker_events_land_via_poll() {
    let mut state = WorkflowState::new();
    state.config.entry = String::from("avaliar o serviço");
    assert!(!state.busy);

    state.start(|id, tx| {
        assert_eq!(id, 1, "primeiro fluxo recebe id 1");
        tx.send(WorkflowEvent::Stage(StageOut {
            stage: String::from("A"),
            task_id: String::from("t-a"),
            latency_ms: 500,
            preview: String::from("análise"),
        }))
        .expect("envia estágio");
        tx.send(WorkflowEvent::Done {
            total_ms: 1_600,
            error: None,
        })
        .expect("envia done");
    });
    assert!(state.busy, "run marca busy imediatamente");
    assert!(state.stages.is_empty());

    poll_until_idle(&mut state);
    assert_eq!(state.stages.len(), 1);
    assert_eq!(state.stages[0].stage, "A");
    assert_eq!(state.total_ms, Some(1_600));
    assert!(state.error.is_none());
}

#[test]
fn worker_death_without_done_releases_ui_with_error() {
    let mut state = WorkflowState::new();
    state.start(|_id, _tx| {
        // sai sem enviar nada: canal desconecta
    });
    poll_until_idle(&mut state);
    assert_eq!(
        state.error.as_deref(),
        Some("worker do workflow terminou sem Done")
    );
}

#[test]
fn observe_applies_events_without_worker() {
    let mut state = WorkflowState::new();
    state.busy = true;
    state.observe(WorkflowEvent::Stage(StageOut {
        stage: String::from("C"),
        task_id: String::from("t-c"),
        latency_ms: 700,
        preview: String::from("final"),
    }));
    assert!(state.busy, "Stage não encerra");
    state.observe(WorkflowEvent::Done {
        total_ms: 2_000,
        error: Some(String::from("timeout no estágio C")),
    });
    assert!(!state.busy);
    assert_eq!(state.error.as_deref(), Some("timeout no estágio C"));
}

#[test]
fn preview_flattens_and_truncates() {
    assert_eq!(
        WorkflowState::preview("linha1\nlinha2", 40),
        "linha1 linha2"
    );
    let long = "x".repeat(100);
    let got = WorkflowState::preview(&long, 10);
    assert_eq!(got.chars().count(), 11); // 10 + reticência
    assert!(got.ends_with('…'));
}

/// Segundo fluxo enquanto busy é ignorado (não sobrepõe corrida).
#[test]
fn start_is_noop_while_busy() {
    use std::sync::atomic::{AtomicUsize, Ordering};
    use std::sync::Arc;
    let spawned = Arc::new(AtomicUsize::new(0));
    let mut state = WorkflowState::new();
    let counter = Arc::clone(&spawned);
    state.start(move |_id, tx| {
        counter.fetch_add(1, Ordering::SeqCst);
        let _ = tx.send(WorkflowEvent::Done {
            total_ms: 1,
            error: None,
        });
    });
    // ainda busy (poll não chamado): segundo start NÃO cria worker novo
    state.start(|_id, _tx| panic!("não deveria despachar"));
    poll_until_idle(&mut state);
    assert_eq!(spawned.load(Ordering::SeqCst), 1);
}
