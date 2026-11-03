//! 3.5 Agentes: verificação headless da tela refeita — fontes DDS/HTTP,
//! Seção A (AgentRegistry ao vivo), Seção B (métricas HTTP + agregado) e
//! snapshot. O caminho HTTP usa stub axum real (`refresh` + `poll`).

use axum::{routing::get, Json, Router};
use eframe::egui;
use egui_kittest::kittest::Queryable;
use orchestrator_studio::agents::AgentsState;
use orchestrator_studio::discovery::{AgentRow, DiscoveryState};
use orchestrator_studio::views;

fn agent(id: &str, model: &str, completed: u64, failed: u64, ema: f32) -> serde_json::Value {
    serde_json::json!({
        "agent_id": id,
        "model": model,
        "specialization": "test",
        "hostname": "stub",
        "health": 100,
        "slots_busy": 1,
        "slots_total": 2,
        "completed_total": completed,
        "failed_total": failed,
        "ema_latency_ms": ema,
    })
}

async fn live_base_url() -> String {
    let app = Router::new().route(
        "/api/v1/agents",
        get(|| async {
            Json(serde_json::json!({
                "agents": [
                    agent("http-11", "mod-h1", 3, 1, 100.0),
                    agent("http-22", "mod-h2", 1, 0, 300.0),
                ],
            }))
        }),
    );
    let listener = tokio::net::TcpListener::bind("127.0.0.1:0")
        .await
        .expect("loopback deve ligar");
    let addr = listener.local_addr().expect("endereco local legivel");
    tokio::spawn(async move {
        axum::serve(listener, app)
            .await
            .expect("stub de teste serve");
    });
    format!("http://{addr}")
}

/// Estado zerado: fontes, seções vazias, botões e queda simulada.
#[test]
fn agents_renders_empty_sources_and_outage_path() {
    let mut agents = AgentsState::new();
    let discovery = DiscoveryState::new_disabled(170);

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (agents, discovery)| views::agents::show(ui, agents, discovery),
        (&mut agents, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label_contains("Topologia Híbrida");
    harness.get_by_label_contains("FONTE 1: DDS RTPS");
    harness.get_by_label_contains("FONTE 2: HTTP :8080 (NÃO LIDO)");
    harness.get_by_label_contains("SEMÂNTICA DUAL");
    harness.get_by_label_contains("Seção A: Agentes Ativos no Domínio DDS");
    harness.get_by_label_contains("0 Agentes Registrados no DDS");
    harness.get_by_label_contains("Nenhum agente com heartbeat");
    harness.get_by_label_contains("Seção B: Métricas de Execução do Orquestrador HTTP");
    harness.get_by_label_contains("Último Polling: —");
    harness.get_by_label_contains("Nenhum agente listado");
    harness.get_by_label("Atualizar Orquestrador");
    harness.get_by_label("Limpar Estatísticas");
    harness.get_by_label("Baixar Snapshot JSON");

    // Queda simulada exercita o caminho de falha sem rede.
    harness.get_by_label("Simular Queda :8080").click();
    harness.run_steps(3);
    harness.get_by_label_contains("queda simulada pelo operador");
    harness.get_by_label_contains("FONTE 2: HTTP :8080 (FALHOU)");
}

/// Seção A viva (AgentRegistry) + Seção B via stub: taxas, agregado e
/// exportação funcional.
#[tokio::test]
async fn agents_populated_renders_rates_aggregate_and_exports() {
    let url = live_base_url().await;
    let mut agents = AgentsState::new();
    agents.url = url.clone();
    agents.refresh();
    while agents.busy {
        agents.poll();
        tokio::time::sleep(std::time::Duration::from_millis(5)).await;
    }
    assert_eq!(agents.list.len(), 2, "stub anuncia 2 agentes");

    let mut discovery = DiscoveryState::new_disabled(170);
    discovery.observe_system(
        Vec::new(),
        vec![AgentRow {
            agent_id: String::from("dds-01"),
            model: String::from("mod-ds"),
            slots_busy: 1,
            slots_total: 2,
            ema_latency_ms: 142.0,
            last_update_ns: orchestrator_studio::machines::now_unix_ns() - 2_000_000_000,
        }],
        Vec::new(),
    );

    let mut harness = egui_kittest::Harness::new_ui_state(
        |ui, (agents, discovery)| views::agents::show(ui, agents, discovery),
        (&mut agents, &discovery),
    );
    harness.set_size(egui::vec2(1400.0, 2200.0));
    harness.run_steps(5);

    harness.get_by_label_contains("1 Agentes Registrados no DDS");
    harness.get_by_label_contains("dds-01");
    harness.get_by_label_contains("1/2 em uso");
    harness.get_by_label_contains("142 ms");
    // Idade exata varia na fronteira de segundo — formato é determinístico.
    harness.get_by_label_contains("s atrás");
    harness.get_by_label_contains("/30s");
    harness.get_by_label_contains("[VIVO]");
    harness.get_by_label_contains("http-11");
    harness.get_by_label_contains("http-22");
    harness.get_by_label_contains("75.00%");
    harness.get_by_label_contains("100.00%");
    harness.get_by_label_contains("Último Polling: ");
    harness.get_by_label_contains("TOTAL AGREGADO (ENXAME)");
    harness.get_by_label_contains("4 completadas");
    harness.get_by_label_contains("1 falhas");
    harness.get_by_label_contains("80.00% confiabilidade");
    harness.get_by_label_contains("2 agente(s) no pool");

    // Snapshot exporta a lista real; limpar zera a Seção B. Os botões da
    // linha de ações ficam abaixo da dobra no harness — `click()` por
    // coordenada não alcança; `click_accesskit()` é a API suportada para
    // widgets fora da área visível (egui_kittest 0.36.2, node.rs).
    harness
        .get_by_label("Baixar Snapshot JSON")
        .click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("snapshot exportado:");
    harness
        .get_by_label("Limpar Estatísticas")
        .click_accesskit();
    harness.run_steps(3);
    harness.get_by_label_contains("Nenhum agente listado");
}
