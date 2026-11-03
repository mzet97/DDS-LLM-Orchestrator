//! 3.6 Despacho: verificação headless da tela refeita — parâmetros reais,
//! card de resultado com T1–T6, abas (formatada/bruto/headers), histórico e
//! Re-enviar/RESET. O fio usa stub axum real; o contador de tasks prova que
//! Re-enviar despacha DE NOVO (task nova no histórico).

use axum::{routing::post, Extension, Json, Router};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::discovery::{AgentRow, DiscoveryState};
use orchestrator_studio::views;
use orchestrator_studio::workload::DispatchState;
use std::sync::{
    atomic::{AtomicUsize, Ordering},
    Arc,
};

/// Stub do `/sync`: `falho` → failed; senão completed com eco dos
/// parâmetros e task alternada por chamada (prova de re-despacho).
fn stub(counter: Arc<AtomicUsize>) -> Router {
    Router::new()
        .route(
            "/api/v1/chat/completions/sync",
            post(
                |Extension(counter): Extension<Arc<AtomicUsize>>,
                 Json(body): Json<serde_json::Value>| async move {
                    let model = body["model"].as_str().unwrap_or("").to_string();
                    if model == "falho" {
                        return Json(serde_json::json!({
                            "task_id": "tsk_fail_01",
                            "status": "failed",
                            "error": "sem agente",
                        }));
                    }
                    let call = counter.fetch_add(1, Ordering::SeqCst);
                    let task_id = if call == 0 {
                        "tsk_aaaa_first"
                    } else {
                        "tsk_bbbb_second"
                    };
                    let temp = body["temperature"].to_string();
                    let max = body["max_tokens"].to_string();
                    Json(serde_json::json!({
                        "task_id": task_id,
                        "status": "completed",
                        "assigned_agent": "agent-stub",
                        "latency_ms": 1280,
                        "content": format!("t={temp} m={max}"),
                        "t_serialization_ns": 1_000_000,
                        "t_transport_send_ns": 5_000_000,
                        "t_agent_queue_ns": 14_000_000,
                        "t_inference_ns": 1_266_000_000,
                        "t_transport_return_ns": 5_000_000,
                        "t_deserialization_ns": 1_000_000,
                    }))
                },
            ),
        )
        .layer(Extension(counter))
}

async fn live_base_url() -> String {
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    tokio::spawn(async move {
        axum::serve(listener, stub(Arc::new(AtomicUsize::new(0))))
            .await
            .expect("stub de teste serve");
    });
    format!("http://{addr}")
}

/// Estado zerado: cabeçalho DIAG, parâmetros, chips honestos e vazios.
#[test]
fn dispatch_renders_empty_params_and_honest_chips() {
    let mut dispatch = DispatchState::new();
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dispatch, discovery)| views::dispatch::show(ui, dispatch, discovery),
        (&mut dispatch, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label_contains("3.6 Despacho de Tarefas");
    harness.get_by_label_contains("POST /api/v1/chat/completions/sync");
    // (Badges kit::badge pintam via painter — sem nó accesskit; chips cobertos
    // por teste de unidade conn_chip_* + captura viva.)
    harness.get_by_label("↻ RESET DE DIAGNÓSTICO");
    harness.get_by_label_contains("▶ PARÂMETROS DE DESPACHO");
    harness.get_by_label_contains("HTTP POST DIRETO");
    harness.get_by_label_contains("SELETOR DE AGENTE / MODELO");
    harness.get_by_label_contains("Síncrono (Wait-for-completion)");
    harness.get_by_label_contains("TIMEOUT (MS)");
    harness.get_by_label_contains("TEMPERATURE");
    harness.get_by_label_contains("MAX TOKENS");
    harness.get_by_label_contains("CTRL + ENTER PARA ENVIAR");
    harness.get_by_label("▶ Despachar Task Síncrona");
    harness.get_by_label("Limpar");
    harness.get_by_label_contains("nenhum despacho nesta sessão ainda.");
    harness.get_by_label_contains("HISTÓRICO RECENTE");
    harness.get_by_label_contains("o histórico aparece aqui após o primeiro envio.");
}

/// Fluxo completo: despachar → card + abas + histórico → Re-enviar (task
/// nova) → RESET zera. Temperatura/max vão no fio (eco do stub).
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dispatch_full_flow_tabs_history_resend_reset() {
    let url = live_base_url().await;
    let mut dispatch = DispatchState::new();
    dispatch.url = url.clone();
    dispatch.model = String::from("echo");
    dispatch.prompt = String::from("prove o fio");
    dispatch.temperature = String::from("0.2");
    dispatch.max_tokens = String::from("1024");
    dispatch.timeout_ms = String::from("10000");

    let mut discovery = DiscoveryState::new_disabled(170);
    discovery.observe_system(
        Vec::new(),
        vec![AgentRow {
            agent_id: String::from("dds-01"),
            model: String::from("mod-ds"),
            slots_busy: 0,
            slots_total: 2,
            ema_latency_ms: 50.0,
            last_update_ns: orchestrator_studio::machines::now_unix_ns(),
        }],
        Vec::new(),
    );

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dispatch, discovery)| views::dispatch::show(ui, dispatch, discovery),
        (&mut dispatch, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    // Despacha de verdade contra o stub (abaixo da dobra: accesskit).
    harness
        .get_by_label("▶ Despachar Task Síncrona")
        .click_accesskit();
    harness.run_steps(40);

    harness.get_by_label_contains("TAREFA CONCLUÍDA (Status: 200 OK)");
    harness.get_by_label_contains("ID: tsk_aaaa_first");
    // "1280 ms" sozinho casa 4 nós (card + TOTAL + histórico + média).
    harness.get_by_label_contains("Fila: 14 ms | Inf: 1266 ms");
    harness
        .get_by_label_contains("TOTAL 1280 ms = FILA 14 + GERAÇÃO 1266 + TRANSPORTE 10 + SERIAL 2");
    // Aba 0 (default): eco dos parâmetros prova temp/max no fio.
    harness.get_by_label_contains("t=0.2 m=1024");
    // Aba 1: corpo bruto (chave só existe no JSON).
    harness.get_by_label("Payload JSON Bruto").click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("assigned_agent");
    // Aba 2: headers reais do stub.
    harness
        .get_by_label("Headers & Telemetria")
        .click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("HTTP 200");
    harness.get_by_label_contains("content-type");
    // Histórico: 1 linha (short ID só existe na tabela).
    harness.get_by_label_contains("TAXA DE SUCESSO: 100%");
    harness.get_by_label_contains("MÉDIA LATÊNCIA: 1280 ms");
    harness.get_by_label("tsk_aaaa_fir…");
    harness.get_by_label("200 OK");

    // Re-enviar despacha DE NOVO: stub devolve a 2ª task.
    harness.get_by_label("Re-enviar").click_accesskit();
    harness.run_steps(40);
    harness.get_by_label("tsk_bbbb_sec…");

    // RESET zera card + histórico.
    harness
        .get_by_label("↻ RESET DE DIAGNÓSTICO")
        .click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("nenhum despacho nesta sessão ainda.");
    harness.get_by_label_contains("o histórico aparece aqui após o primeiro envio.");
}

/// Temperatura inválida barra sem rede (banner honesto sob os botões).
#[test]
fn dispatch_invalid_temperature_blocks() {
    let mut dispatch = DispatchState::new();
    dispatch.prompt = String::from("oi");
    dispatch.temperature = String::from("quente");
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dispatch, discovery)| views::dispatch::show(ui, dispatch, discovery),
        (&mut dispatch, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness
        .get_by_label("▶ Despachar Task Síncrona")
        .click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("temperatura inválida");
}

/// Falha do backend: card vermelho + motivo + linha "falha" no histórico.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn dispatch_backend_failure_card() {
    let url = live_base_url().await;
    let mut dispatch = DispatchState::new();
    dispatch.url = url;
    dispatch.model = String::from("falho");
    dispatch.prompt = String::from("vai falhar");
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (dispatch, discovery)| views::dispatch::show(ui, dispatch, discovery),
        (&mut dispatch, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness
        .get_by_label("▶ Despachar Task Síncrona")
        .click_accesskit();
    harness.run_steps(40);

    harness.get_by_label_contains("TAREFA FALHOU");
    harness.get_by_label_contains("motivo do backend: sem agente");
    harness.get_by_label("falha");
    harness.get_by_label_contains("TAXA DE SUCESSO: 0%");
}
