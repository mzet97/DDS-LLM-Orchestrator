//! Testes unitários do orchestrator: scheduler (T-402) + selector (T-404).

use dds_contract::generated::dds_llm_orchestrator::{AgentState, Task};
use orchestrator::{select_agent, Scheduler};

fn make_task(id: &str, priority: i32, created: u64, model_required: i32) -> Task {
    Task {
        task_id: id.into(),
        client_id: "c".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required,
        model_name: "qwen".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 8,
        stream: false,
        status: 0,
        priority,
        created_at_ns: created,
        assigned_at_ns: 0,
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns: created + 60_000_000_000,
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

fn make_agent(id: &str, spec: &str, busy: u32, health: i32) -> AgentState {
    AgentState {
        agent_id: id.into(),
        hostname: "h".into(),
        model: "qwen".into(),
        specialization: spec.into(),
        slots_total: 4,
        slots_busy: busy,
        vram_total_mb: 0,
        vram_used_mb: 0,
        ema_latency_ms: 0.0,
        completed_total: 0,
        failed_total: 0,
        health,
        last_update_ns: 1,
        uptime_seconds: 1,
    }
}

#[test]
fn t402_scheduler_ordem_prioridade_depois_idade() {
    let mut s = Scheduler::new();
    s.push(make_task("low-old", 1, 100, 0));
    s.push(make_task("high-new", 10, 300, 0));
    s.push(make_task("high-old", 10, 100, 0));
    s.push(make_task("mid", 5, 200, 0));

    assert_eq!(
        s.pop().unwrap().task_id,
        "high-old",
        "maior prioridade, mais antigo"
    );
    assert_eq!(
        s.pop().unwrap().task_id,
        "high-new",
        "maior prioridade, mais novo"
    );
    assert_eq!(s.pop().unwrap().task_id, "mid");
    assert_eq!(s.pop().unwrap().task_id, "low-old");
    assert!(s.pop().is_none());
}

#[test]
fn t404_selector_roteamento_por_especializacao() {
    let agents = vec![
        make_agent("text-1", "TEXT", 0, 2),
        make_agent("vision-1", "VISION", 0, 2),
        make_agent("vision-busy", "VISION", 4, 2), // sem slots
        make_agent("vision-off", "VISION", 0, 0),  // OFFLINE
    ];

    // Task VISION: text-1 (TEXT aceita qualquer coisa) e vision-1 são elegíveis;
    // com carga igual, least-loaded devolve qualquer um dos dois (empate válido).
    let t = make_task("t1", 5, 100, 1);
    let chosen = select_agent(&t, &agents).expect("deve escolher");
    assert!(
        ["text-1", "vision-1"].contains(&chosen.agent_id.as_str()),
        "escolha deve ser elegível, veio {}",
        chosen.agent_id
    );

    // Task TEXT: TEXT aceita qualquer spec — least-loaded vence (todos com 0 busy;
    // min_by_key devolve o primeiro com 0)
    let t = make_task("t2", 5, 100, 0);
    let chosen = select_agent(&t, &agents).expect("deve escolher");
    assert!(["text-1", "vision-1"].contains(&chosen.agent_id.as_str()));

    // Task EMBEDDING (required=2): TEXT aceita tudo → text-1 é elegível;
    // os agentes VISION rejeitam (só aceitam TEXT/VISION).
    let t = make_task("t3", 5, 100, 2);
    let chosen = select_agent(&t, &agents).expect("TEXT deve aceitar EMBEDDING");
    assert_eq!(chosen.agent_id, "text-1");

    // Rejeição total: só VISION disponível para uma task EMBEDDING → None
    let agents_so_vision = vec![make_agent("vision-1", "VISION", 0, 2)];
    let t = make_task("t4", 5, 100, 2);
    assert!(select_agent(&t, &agents_so_vision).is_none());

    // Vision indisponível (slots cheios): task VISION sem elegível → None
    let agents_sem_vision = vec![make_agent("vision-busy", "VISION", 4, 2)];
    let t = make_task("t5", 5, 100, 1);
    assert!(select_agent(&t, &agents_sem_vision).is_none());
}
