//! 3.3 Inferência & Chat de Engenharia: verificação headless da tela
//! refeita — cabeçalho com planos, descoberta ServerStatus, endpoint + probe,
//! parâmetros, transcript com stats e composer. O caminho vivo usa stub HTTP
//! real (axum em porta efêmera): `refresh_models`/`send` + `poll` de verdade,
//! sem dados inventados.

use axum::{
    routing::{get, post},
    Json, Router,
};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::discovery::{DiscoveryState, ServerRow};
use orchestrator_studio::inference::InferenceState;
use orchestrator_studio::views;

fn stub() -> Router {
    Router::new()
        .route(
            "/v1/models",
            get(|| async {
                Json(serde_json::json!({"data": [{"id": "modelo-a"}, {"id": "modelo-b"}]}))
            }),
        )
        .route(
            "/v1/chat/completions",
            post(|| async {
                Json(serde_json::json!({
                    "choices": [{"message": {"role": "assistant", "content": "resposta-prova"}}],
                    "usage": {"prompt_tokens": 10, "completion_tokens": 5},
                }))
            }),
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

/// Estado zerado: cabeçalho, quórum vazio, endpoint sem verificação,
/// parâmetros, transcript vazio e composer.
#[test]
fn inference_renders_header_empty_discovery_and_composer() {
    let mut infer = InferenceState::new();
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (infer, discovery)| views::inference::show(ui, infer, discovery),
        (&mut infer, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label_contains("3.3 Inferência & Chat de Engenharia");
    harness.get_by_label_contains("PLANO DDS: NENHUM SERVIDOR VISTO");
    harness.get_by_label_contains("PLANO HTTP: ROTAS V1 NÃO VERIFICADAS");
    harness.get_by_label_contains("DESCOBERTA DE INFERÊNCIA NO DOMÍNIO");
    harness.get_by_label_contains("Nenhum servidor anunciando ServerStatus");
    harness.get_by_label_contains("Postura de acoplamento");
    harness.get_by_label_contains("ENDPOINT DE CONTROLE HTTP & MODEL PROBE");
    harness.get_by_label_contains("Sem verificação (informe a URL e clique Verificar)");
    harness.get_by_label_contains("PARÂMETROS DE GERAÇÃO (QOS T-820)");
    harness.get_by_label_contains("TEMPERATURA");
    harness.get_by_label_contains("Nucleus sampling");
    harness.get_by_label_contains("120 seg (fixo)");
    harness.get_by_label_contains("TRANSCRIPT DE ENGENHARIA & DEPURAÇÃO");
    harness.get_by_label_contains("SESSÃO VAZIA");
    harness.get_by_label_contains("sessão vazia — envie o primeiro prompt abaixo.");
    harness.get_by_label_contains("0 caracteres");
    harness.get_by_label_contains("Atalho: Ctrl + Enter para enviar");
    harness.get_by_label("▶ ENVIAR PROMPT");
    harness.get_by_label("Parar Geração");
    harness.get_by_label("Limpar Histórico");
    harness.get_by_label("Exportar JSON");
}

/// Servidor verificado + turno real via stub: planos ativos, linha HTTP 200,
/// transcript com stats e exportação funcional.
#[tokio::test]
async fn inference_verified_turn_renders_plans_stats_and_exports() {
    let url = live_base_url().await;
    let mut infer = InferenceState::new();
    infer.server_url = url.clone();
    infer.refresh_models();
    while infer.busy {
        infer.poll();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(infer.models.len(), 2, "stub anuncia 2 modelos");
    infer.prompt = String::from("diga OK");
    infer.send();
    while infer.busy {
        infer.poll();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(infer.history.len(), 2, "turno completo no histórico");

    let mut discovery = DiscoveryState::new_disabled(170);
    discovery.observe_system(
        Vec::new(),
        Vec::new(),
        vec![ServerRow {
            server_id: String::from("llama-stub-01"),
            model_loaded: String::from("modelo-srv"),
            slots_idle: 3,
            slots_processing: 1,
            ready: true,
        }],
    );

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (infer, discovery)| views::inference::show(ui, infer, discovery),
        (&mut infer, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label_contains("PLANO DDS: PARTICIPANTE VISTO");
    harness.get_by_label_contains("PLANO HTTP: ROTAS V1 ATIVAS");
    harness.get_by_label_contains("llama-stub-01");
    harness.get_by_label_contains("modelo-srv");
    harness.get_by_label_contains("1/4 ATIVOS (3 LIVRES)");
    harness.get_by_label_contains("Pronto (lease 10s do tópico)");
    harness.get_by_label_contains("HTTP 200 OK");
    harness.get_by_label_contains("2 modelo(s): modelo-a · modelo-b");
    harness.get_by_label_contains("assistente (modelo-a)");
    harness.get_by_label_contains("você (operador local)");
    harness.get_by_label_contains("diga OK");
    harness.get_by_label_contains("resposta-prova");
    harness.get_by_label_contains("10+5 tokens");
    harness.get_by_label_contains("TOKENS NA SESSÃO (USAGE REAL): 15");

    // Exportar JSON escreve o transcript real e mostra o caminho.
    harness.get_by_label("Exportar JSON").click();
    harness.run_steps(3);
    harness.get_by_label_contains("transcript exportado:");
}
