use super::*;

#[test]
fn agent_row_maps_contract_fields() {
    let state = AgentState {
        agent_id: String::from("a-1"),
        model: String::from("m"),
        slots_busy: 2,
        slots_total: 8,
        ..AgentState::default()
    };

    assert_eq!(
        agent_row(&state),
        AgentRow {
            agent_id: String::from("a-1"),
            model: String::from("m"),
            slots_busy: 2,
            slots_total: 8,
        }
    );
}

#[test]
fn tool_row_maps_contract_fields() {
    let call = ToolCallRequest {
        call_id: String::from("c-1"),
        tool_name: String::from("ler_arquivo"),
        status: 3,
        ..ToolCallRequest::default()
    };

    assert_eq!(
        tool_row(&call),
        ToolRow {
            call_id: String::from("c-1"),
            tool_name: String::from("ler_arquivo"),
            status: 3,
            requester_id: String::new(),
            security_level: 0,
            result_preview: String::new(),
            arguments_json: String::new(),
            result_json: String::new(),
            duration_ms: 0,
        }
    );
}

/// PRD 3.13: duração real = completed−created, e 0 enquanto aberta.
#[test]
fn tool_row_derives_duration_from_wire_timestamps() {
    let call = ToolCallRequest {
        call_id: String::from("c-3"),
        created_at_ns: 1_000_000_000,
        completed_at_ns: 2_500_000_000,
        ..ToolCallRequest::default()
    };
    assert_eq!(tool_row(&call).duration_ms, 1_500);
}

/// PRD 3.13: payloads íntegros atravessam o mapeamento (cap 8 KB).
#[test]
fn tool_row_keeps_full_payloads_capped() {
    let args = format!("{{\"path\":\"{}\"}}", "x".repeat(9_000));
    let call = ToolCallRequest {
        call_id: String::from("c-4"),
        arguments_json: args.clone(),
        result_json: String::from("{\"ok\":true}"),
        ..ToolCallRequest::default()
    };
    let row = tool_row(&call);
    assert_eq!(row.result_json, "{\"ok\":true}");
    assert!(row.arguments_json.len() < args.len());
    assert!(row.arguments_json.ends_with('…'));
}

#[test]
fn task_row_maps_contract_fields() {
    let task = Task {
        task_id: String::from("t-8f21a4"),
        client_id: String::from("studio"),
        assigned_agent: String::from("agent-1"),
        model_name: String::from("qwen3"),
        status: 2,
        priority: 5,
        retry_count: 1,
        created_at_ns: 100,
        completed_at_ns: 0,
        ..Task::default()
    };

    let row = task_row(&task);
    assert_eq!(row.task_id, "t-8f21a4");
    assert_eq!(row.status, 2);
    assert_eq!(task_status_label(row.status), "RUNNING");
    assert_eq!(task_status_label(3), "DONE");
    assert_eq!(task_status_label(4), "FAILED");
    assert_eq!(task_status_label(9), "desconhecido");
}

#[test]
fn task_output_row_maps_contract_fields() {
    let output = TaskOutput {
        task_id: String::from("t-1"),
        seq_num: 2,
        agent_id: String::from("agent-1"),
        is_final: false,
        token_count: 42,
        emitted_at_ns: 999,
        content: String::from("resposta parcial"),
        ..TaskOutput::default()
    };

    let row = task_output_row(&output);
    assert_eq!(row.seq_num, 2);
    assert_eq!(row.token_count, 42);
    assert_eq!(row.content, "resposta parcial");
}

/// PRD 3.13: vocabulário de nível da GUI (N0/N1/N2).
#[test]
fn security_level_labels_follow_prd() {
    assert_eq!(security_level_label(0), "N0 READ_ONLY");
    assert_eq!(security_level_label(1), "N1 SANDBOX_EXEC");
    assert_eq!(security_level_label(2), "N2 HOST_MUTATION");
    assert_eq!(security_level_label(7), "N7 ?");
}

/// T-890-06: requester/nível/result previa atravessam o mapeamento,
/// com resultado truncado para exibição.
#[test]
fn tool_row_maps_governance_and_truncates_result() {
    let long = "x".repeat(200);
    let call = ToolCallRequest {
        call_id: String::from("c-2"),
        tool_name: String::from("filesystem.read_file"),
        requester_id: String::from("agent-1"),
        security_level: 2,
        result_json: format!("{{\"conteudo\":\"{long}\"}}"),
        status: 3,
        ..ToolCallRequest::default()
    };

    let row = tool_row(&call);
    assert_eq!(row.requester_id, "agent-1");
    assert_eq!(row.security_level, 2);
    assert!(row.result_preview.ends_with('…'));
    assert!(row.result_preview.chars().count() <= 81);
}

#[test]
fn discovery_row_maps_contract_fields() {
    let event = DiscoveryEvent {
        event_type: String::from("novo"),
        topic_name: String::from("Tasks"),
        remote_entity: String::from("guid-1"),
        ..DiscoveryEvent::default()
    };

    assert_eq!(
        discovery_row(&event),
        DiscoveryRow {
            event_type: String::from("novo"),
            topic_name: String::from("Tasks"),
            remote_entity: String::from("guid-1"),
        }
    );
}

#[test]
fn metric_row_formats_name_with_unit() {
    let metric = SystemMetric {
        component_id: String::from("no-a"),
        metric_name: String::from("cpu"),
        value: 0.5,
        unit: String::from("fração"),
        ..SystemMetric::default()
    };

    assert_eq!(
        metric_row(&metric),
        MetricRow {
            source: String::from("no-a"),
            name: String::from("cpu (fração)"),
            value: 0.5,
        }
    );
}

// T-890: a linha de nó Studio carrega identidade + modo de acesso.
#[test]
fn studio_node_row_maps_contract_fields() {
    let presence = StudioNodePresence {
        node_id: String::from("lab-1:4317"),
        url: String::from("http://127.0.0.1:4317"),
        token_required: true,
        ..StudioNodePresence::default()
    };

    assert_eq!(
        studio_node_row(&presence),
        StudioNodeRow {
            node_id: String::from("lab-1:4317"),
            url: String::from("http://127.0.0.1:4317"),
            token_required: true,
        }
    );
}
