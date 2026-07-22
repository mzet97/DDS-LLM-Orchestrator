//! # Orchestrator — Control Plane (Fase 3)
//!
//! API HTTP (axum) + scheduler + registry + selector + NFCM.
//!
//! ## Uso
//! ```bash
//! CYCLONEDDS_STATIC=1 cargo run -p orchestrator --features dds -- \
//!     --port 8080 --dds-domain 0 --qos-manager nfcm --fuzzy-routing
//! ```
//! `--fuzzy-routing` (opcional, default OFF): publica `QoS.RoutingProfile`
//! quando o decisor de QoS troca de perfil — porte de `--fuzzy-routing` do
//! `main.py`.

// `not(test)`: no binário de teste (--all-targets) o harness gerado pelo
// rustc substitui `fn main` — sem isso, o módulo inteiro fica "never used"
// (dead_code) porque nada no crate de teste chama `app::main`/os handlers.
#[cfg(all(feature = "dds", not(test)))]
mod app {
    use axum::{
        extract::State,
        http::StatusCode,
        response::Json,
        routing::{get, post},
        Router,
    };
    use dds_contract::generated::dds_llm_orchestrator::Task;
    use orchestrator::dds::OrchestratorDds;
    use serde::{Deserialize, Serialize};
    use std::sync::Arc;
    use std::time::Duration;

    #[derive(Clone)]
    struct AppState {
        orch: Arc<OrchestratorDds>,
    }

    #[derive(Deserialize)]
    pub struct ChatRequest {
        model: String,
        messages: Vec<Message>,
        temperature: Option<f32>,
        max_tokens: Option<u32>,
        stream: Option<bool>,
    }

    #[derive(Deserialize, Serialize)]
    pub struct Message {
        role: String,
        content: String,
    }

    #[derive(Serialize)]
    struct ChatResponse {
        task_id: String,
        status: String,
    }

    #[derive(Serialize)]
    struct ErrorResponse {
        error: String,
    }

    /// POST /api/v1/chat/completions — publica Task no tópico Tasks (T-401).
    async fn submit_task(
        State(state): State<AppState>,
        Json(req): Json<ChatRequest>,
    ) -> Result<Json<ChatResponse>, (StatusCode, Json<ErrorResponse>)> {
        let task_id = uuid::Uuid::new_v4().to_string();
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let messages_json = serde_json::to_string(&req.messages).unwrap_or_default();

        let task = Task {
            task_id: task_id.clone(),
            client_id: "http-client".into(),
            assigned_agent: String::new(),
            target_agent: String::new(),
            model_required: 0, // TEXT
            model_name: req.model,
            messages_json,
            temperature: req.temperature.unwrap_or(0.7),
            max_tokens: req.max_tokens.unwrap_or(256),
            stream: req.stream.unwrap_or(false),
            status: 0,   // PENDING
            priority: 5, // NORMAL
            created_at_ns: now_ns,
            assigned_at_ns: 0,
            started_at_ns: 0,
            completed_at_ns: 0,
            deadline_ns: now_ns + 120_000_000_000, // +120s
            retry_count: 0,
            finish_reason: String::new(),
            t_serialization_ns: 0,
            t_transport_send_ns: 0,
            t_agent_queue_ns: 0,
            t_inference_ns: 0,
            t_transport_return_ns: 0,
            t_deserialization_ns: 0,
        };

        state.orch.publish_task(task).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

        tracing::info!(task_id = %task_id, "task publicada via API");
        Ok(Json(ChatResponse {
            task_id,
            status: "pending".into(),
        }))
    }

    /// POST /api/v1/chat/completions/sync — publica Task e aguarda resultado.
    async fn submit_task_sync(
        State(state): State<AppState>,
        Json(req): Json<ChatRequest>,
    ) -> Result<Json<serde_json::Value>, (StatusCode, Json<ErrorResponse>)> {
        let task_id = uuid::Uuid::new_v4().to_string();
        let now_ns = std::time::SystemTime::now()
            .duration_since(std::time::UNIX_EPOCH)
            .unwrap_or_default()
            .as_nanos() as u64;

        let messages_json = serde_json::to_string(&req.messages).unwrap_or_default();

        let task = Task {
            task_id: task_id.clone(),
            client_id: "http-client".into(),
            assigned_agent: String::new(),
            target_agent: String::new(),
            model_required: 0,
            model_name: req.model,
            messages_json,
            temperature: req.temperature.unwrap_or(0.7),
            max_tokens: req.max_tokens.unwrap_or(256),
            stream: req.stream.unwrap_or(false),
            status: 0,
            priority: 5,
            created_at_ns: now_ns,
            assigned_at_ns: 0,
            started_at_ns: 0,
            completed_at_ns: 0,
            deadline_ns: now_ns + 120_000_000_000,
            retry_count: 0,
            finish_reason: String::new(),
            t_serialization_ns: 0,
            t_transport_send_ns: 0,
            t_agent_queue_ns: 0,
            t_inference_ns: 0,
            t_transport_return_ns: 0,
            t_deserialization_ns: 0,
        };

        state.orch.publish_task(task).await.map_err(|e| {
            (
                StatusCode::INTERNAL_SERVER_ERROR,
                Json(ErrorResponse {
                    error: e.to_string(),
                }),
            )
        })?;

        // Aguarda resultado via polling do DataSpace
        let deadline = std::time::Instant::now() + Duration::from_secs(120);
        loop {
            if std::time::Instant::now() > deadline {
                return Ok(Json(serde_json::json!({
                    "task_id": task_id,
                    "status": "timeout",
                    "error": " inference timeout after 120s"
                })));
            }

            // Lê task do cache (upsert monotônico, sem cap de leitura — ver
            // o comentário em `agent::dds::attempt_claim_and_process` sobre
            // por que `read_task_mesh` satura em ~65 tasks processadas).
            if let Some(current) = state.orch.dataspace().caches().read_task(&task_id) {
                match current.status {
                    3 => {
                        // DONE
                        let latency_ns = current
                            .completed_at_ns
                            .saturating_sub(current.created_at_ns);
                        let latency_ms = latency_ns / 1_000_000;
                        return Ok(Json(serde_json::json!({
                            "task_id": task_id,
                            "status": "completed",
                            "latency_ms": latency_ms,
                            "finish_reason": current.finish_reason,
                            "assigned_agent": current.assigned_agent,
                            "tokens_prompt": 0,
                            "tokens_completion": 0,
                        })));
                    }
                    4 => {
                        // FAILED
                        return Ok(Json(serde_json::json!({
                            "task_id": task_id,
                            "status": "failed",
                            "error": current.finish_reason,
                        })));
                    }
                    _ => {}
                }
            }

            tokio::time::sleep(Duration::from_millis(50)).await;
        }
    }

    /// GET /health — health check.
    async fn health() -> Json<serde_json::Value> {
        Json(serde_json::json!({
            "status": "ok",
            "component": "orchestrator",
            "timestamp": chrono::Utc::now().to_rfc3339(),
        }))
    }

    /// GET /api/v1/agents — lista agentes do registry.
    async fn list_agents(State(state): State<AppState>) -> Json<serde_json::Value> {
        let agents: Vec<serde_json::Value> = state
            .orch
            .registry()
            .all()
            .iter()
            .map(|a| {
                serde_json::json!({
                    "agent_id": a.agent_id,
                    "hostname": a.hostname,
                    "model": a.model,
                    "specialization": a.specialization,
                    "slots_total": a.slots_total,
                    "slots_busy": a.slots_busy,
                    "vram_total_mb": a.vram_total_mb,
                    "vram_used_mb": a.vram_used_mb,
                    "ema_latency_ms": a.ema_latency_ms,
                    "completed_total": a.completed_total,
                    "failed_total": a.failed_total,
                    "health": a.health,
                    "last_update_ns": a.last_update_ns,
                    "uptime_seconds": a.uptime_seconds,
                })
            })
            .collect();
        Json(serde_json::json!({ "agents": agents, "count": agents.len() }))
    }

    #[tokio::main]
    pub async fn main() -> anyhow::Result<()> {
        tracing_subscriber::fmt()
            .with_env_filter(
                tracing_subscriber::EnvFilter::try_from_default_env()
                    .unwrap_or_else(|_| "info".into()),
            )
            .init();

        let mut port: u16 = 8080;
        let mut domain: u32 = 0;
        let mut qos_manager = "nfcm".to_string();
        let mut fuzzy_routing = false;
        let args: Vec<String> = std::env::args().collect();
        let mut i = 1;
        while i < args.len() {
            match args[i].as_str() {
                "--port" => {
                    port = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(8080);
                    i += 2;
                }
                "--dds-domain" => {
                    domain = args.get(i + 1).and_then(|s| s.parse().ok()).unwrap_or(0);
                    i += 2;
                }
                "--qos-manager" => {
                    if let Some(v) = args.get(i + 1) {
                        qos_manager = v.clone();
                    }
                    i += 2;
                }
                // Porte de `--fuzzy-routing` (main.py): flag booleana, sem valor.
                "--fuzzy-routing" => {
                    fuzzy_routing = true;
                    i += 1;
                }
                _ => i += 1,
            }
        }

        // T-504: --qos-manager {static,zadeh,fcm,fcm-dhl,nfcm}
        let decider: std::sync::Arc<dyn qos_nfcm::decider::QosDecider> = match qos_manager.as_str()
        {
            "static" => std::sync::Arc::new(qos_nfcm::decider::StaticDecider::new(
                qos_nfcm::QoSProfile::Balanced,
            )),
            "zadeh" => std::sync::Arc::new(qos_nfcm::zadeh::ZadehDecider::new()),
            "fcm" => std::sync::Arc::new(qos_nfcm::fcm::FcmDecider::new()),
            "fcm-dhl" => std::sync::Arc::new(qos_nfcm::fcm::FcmDhlDecider::default()),
            _ => std::sync::Arc::new(qos_nfcm::Nfcm::qos_default()),
        };

        tracing::info!(qos_manager = %qos_manager, "decisor de QoS selecionado");

        let orch =
            Arc::new(OrchestratorDds::new(domain, decider)?.with_fuzzy_routing(fuzzy_routing));
        let _feeders = orch.spawn_cache_feeders();
        let _registry_monitor =
            orch.spawn_registry_monitor(Duration::from_secs(15), Duration::from_secs(2));
        let _control_loop = orch.spawn_control_loop(Duration::from_secs(2));
        // Porte de `publish_interval_s: float = 5.0` (default do QoSMonitor Python).
        let _qos_monitor = orch.spawn_qos_monitor(Duration::from_secs(5));

        let state = AppState { orch };
        let app = Router::new()
            .route("/health", get(health))
            .route("/api/v1/chat/completions", post(submit_task))
            .route("/api/v1/chat/completions/sync", post(submit_task_sync))
            .route("/api/v1/agents", get(list_agents))
            .with_state(state);

        let addr = format!("0.0.0.0:{port}");
        tracing::info!(addr = %addr, domain, "orchestrator iniciando");

        let listener = tokio::net::TcpListener::bind(&addr).await?;
        axum::serve(listener, app).await?;
        Ok(())
    }
}

#[cfg(all(feature = "dds", not(test)))]
use app::main;

#[cfg(not(feature = "dds"))]
fn main() {
    eprintln!("orchestrator: build sem feature `dds` — nada a fazer (use --features dds)");
}
