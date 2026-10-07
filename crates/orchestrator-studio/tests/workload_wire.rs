//! Despacho contra stub HTTP real em porta efêmera (concluído e falha).

use orchestrator_studio::workload::DispatchOutcome;

fn stub() -> axum::Router {
    axum::Router::new().route(
        "/api/v1/chat/completions/sync",
        axum::routing::post(
            |axum::Json(body): axum::Json<serde_json::Value>| async move {
                let model = body["model"].as_str().unwrap_or("").to_string();
                axum::Json(if model == "falho" {
                    serde_json::json!({"task_id": "t-2", "status": "failed", "error": "sem agente"})
                } else if model == "estranho" {
                    serde_json::json!({"task_id": "t-3", "status": "bizarro"})
                } else if model == "decomp" {
                    // Decomposição T1–T6 real do backend (ns).
                    serde_json::json!({"task_id": "t-4", "status": "completed",
                    "assigned_agent": "agent-teste", "latency_ms": 1500,
                    "t_serialization_ns": 2_000_000, "t_transport_send_ns": 100_000_000,
                    "t_agent_queue_ns": 900_000_000, "t_inference_ns": 400_000_000,
                    "t_transport_return_ns": 50_000_000, "t_deserialization_ns": 1_000_000})
                } else {
                    serde_json::json!({"task_id": "t-1", "status": "completed",
                    "assigned_agent": "agent-teste", "latency_ms": 1234})
                })
            },
        ),
    )
}

async fn live_base_url() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    tokio::spawn(async move {
        axum::serve(listener, stub())
            .await
            .expect("stub de teste serve");
    });
    format!("http://{addr}")
}

#[tokio::test]
async fn completed_returns_agent_and_latency() {
    let url = live_base_url().await;

    let outcome = tokio::task::spawn_blocking(move || {
        orchestrator_studio::workload::dispatch_sync(&url, "m", "oi")
    })
    .await
    .expect("sem panic")
    .expect("stub responde");

    assert_eq!(
        outcome,
        DispatchOutcome::Completed {
            task_id: String::from("t-1"),
            assigned_agent: Some(String::from("agent-teste")),
            latency_ms: 1234,
            content: None,
            breakdown: None,
        }
    );
}

/// PRD 3.6: os seis campos T1–T6 do `/sync` viram a decomposição
/// fila × geração × transporte × serialização (ms).
#[tokio::test]
async fn completed_parses_t_breakdown_fields() {
    let url = live_base_url().await;

    let outcome = tokio::task::spawn_blocking(move || {
        orchestrator_studio::workload::dispatch_sync(&url, "decomp", "oi")
    })
    .await
    .expect("sem panic")
    .expect("stub responde");

    let DispatchOutcome::Completed { breakdown, .. } = outcome else {
        panic!("deve concluir");
    };
    let b = breakdown.expect("decomposição presente");
    assert_eq!(b.queue_ms, 900, "t_agent_queue_ns → fila");
    assert_eq!(b.inference_ms, 400, "t_inference_ns → geração");
    assert_eq!(b.transport_ms, 150, "send+return → transporte");
    assert_eq!(b.serial_ms, 3, "serialize+deserialize → serial");
}

#[tokio::test]
async fn failed_returns_task_error() {
    let url = live_base_url().await;

    let outcome = tokio::task::spawn_blocking(move || {
        orchestrator_studio::workload::dispatch_sync(&url, "falho", "oi")
    })
    .await
    .expect("sem panic")
    .expect("stub responde");

    assert_eq!(
        outcome,
        DispatchOutcome::Failed {
            task_id: String::from("t-2"),
            error: String::from("sem agente"),
        }
    );
}

/// Smoke contra orquestrador real: só roda com `STUDIO_LIVE_DISPATCH=1`.
#[test]
fn live_dispatch_smoke_when_requested() {
    if std::env::var("STUDIO_LIVE_DISPATCH").is_err() {
        return;
    }
    let outcome = orchestrator_studio::workload::dispatch_sync(
        "http://127.0.0.1:8085",
        "qwen3.5-0.8b",
        "responda só: VIVO",
    )
    .expect("orquestrador vivo despacha");
    assert!(
        matches!(outcome, DispatchOutcome::Completed { .. }),
        "agente real conclui, recebido: {outcome:?}"
    );
}

#[tokio::test]
async fn unknown_status_becomes_typed_error() {
    let url = live_base_url().await;

    let err = tokio::task::spawn_blocking(move || {
        orchestrator_studio::workload::dispatch_sync(&url, "estranho", "oi")
    })
    .await
    .expect("sem panic")
    .expect_err("status bizarro deve falhar");

    assert!(matches!(
        err,
        orchestrator_studio::workload::DispatchError::Unreachable { .. }
    ));
}
