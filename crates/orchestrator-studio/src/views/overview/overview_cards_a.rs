//! Parte de `views::overview` (extraída para manter os arquivos revisáveis).

use super::*;

/// Cartão 1 — Nó studio-node (daemon local + protocolo + operações + EMA).
pub(super) fn node_card(
    state: &AppState,
    discovery: &DiscoveryState,
    dot: egui::Color32,
) -> TeleCard {
    let cyan = theme::PRIMARY_FIXED_DIM;
    match state.node() {
        Some(node) => TeleCard {
            dot,
            title: String::from("Nó studio-node"),
            status: String::from("Conectado"),
            status_color: theme::OK,
            subtitle: String::from("Daemon Local · Telemetria Host HTTP"),
            rows: vec![
                (
                    String::from("Protocolo:"),
                    format!("v{}.{}", node.version.major, node.version.minor),
                    cyan,
                ),
                (
                    String::from("Log Operações:"),
                    format!("{} registradas", node.operations.len()),
                    cyan,
                ),
                (
                    String::from("Latência (EMA):"),
                    state
                        .rtt_ema_ms()
                        .map(|ema| format!("{ema:.1}ms"))
                        .unwrap_or_else(|| String::from("—")),
                    cyan,
                ),
            ],
            footer: (
                String::from("Alvo:"),
                discovery
                    .selected_url()
                    .map(|url| kit::short_host(&url))
                    .unwrap_or_else(|| String::from("—")),
            ),
        },
        None => TeleCard {
            dot,
            title: String::from("Nó studio-node"),
            status: String::from("Aguardando"),
            status_color: theme::STALE,
            subtitle: String::from("Daemon Local · Telemetria Host HTTP"),
            rows: vec![
                (String::from("Protocolo:"), String::from("—"), theme::STALE),
                (
                    String::from("Log Operações:"),
                    String::from("—"),
                    theme::STALE,
                ),
                (
                    String::from("Latência (EMA):"),
                    String::from("—"),
                    theme::STALE,
                ),
            ],
            footer: (String::from("Alvo:"), String::from("— nenhum alvo —")),
        },
    }
}

/// Cartão 2 — Serviços systemd (plano pretendido × efetivo + divergências).
pub(super) fn services_card(services: &ServicesPanel, dot: egui::Color32) -> TeleCard {
    let cyan = theme::PRIMARY_FIXED_DIM;
    let list = &services.list;
    let ready = list.iter().filter(|item| item.active).count();
    let divergent = list
        .iter()
        .filter(|item| item.wanted.is_some_and(|wanted| wanted != item.active))
        .count();
    let (status, status_color) = if !services.error.is_empty() {
        (String::from("Falha de leitura"), theme::ERROR)
    } else if list.is_empty() {
        (String::from("Aguardando"), theme::STALE)
    } else if divergent > 0 {
        (String::from("Divergência"), theme::WARN)
    } else {
        (String::from("Sincronizado"), theme::OK)
    };
    let unit_row = list.first().map(|unit| {
        let diverged = unit.wanted.is_some_and(|wanted| wanted != unit.active);
        (
            unit.service.chars().take(20).collect(),
            format!(
                "wanted={}",
                unit.wanted
                    .map(|wanted| wanted.to_string())
                    .unwrap_or_else(|| String::from("—"))
            ),
            if diverged { theme::WARN } else { cyan },
        )
    });
    TeleCard {
        dot,
        title: String::from("Serviços (systemd)"),
        status,
        status_color,
        subtitle: String::from("Plano de Unidades do Sistema Operacional"),
        rows: if list.is_empty() {
            vec![
                (
                    String::from("Unidades Ativas:"),
                    String::from("—"),
                    theme::STALE,
                ),
                (String::from("Unidade:"), String::from("—"), theme::STALE),
                (
                    String::from("Divergentes:"),
                    String::from("—"),
                    theme::STALE,
                ),
            ]
        } else {
            vec![
                (
                    String::from("Unidades Ativas:"),
                    format!("{ready} de {} monitoradas", list.len()),
                    cyan,
                ),
                unit_row
                    .unwrap_or_else(|| (String::from("Unidade:"), String::from("—"), theme::STALE)),
                (
                    String::from("Divergentes:"),
                    divergent.to_string(),
                    if divergent > 0 { theme::WARN } else { cyan },
                ),
            ]
        },
        footer: (String::from("Detalhe:"), String::from("ver tela 3.8")),
    }
}

/// Cartão 3 — Agentes do orquestrador HTTP (+ queda para a descoberta DDS).
pub(super) fn agents_card(
    agents: &AgentsState,
    discovery: &DiscoveryState,
    dot: egui::Color32,
) -> TeleCard {
    let cyan = theme::PRIMARY_FIXED_DIM;
    let (count, busy, total, ema_ms) = if agents.list.is_empty() {
        let busy: u32 = discovery.agents.iter().map(|a| a.slots_busy).sum();
        let total: u32 = discovery.agents.iter().map(|a| a.slots_total).sum();
        let ema = if discovery.agents.is_empty() {
            None
        } else {
            Some(
                discovery
                    .agents
                    .iter()
                    .map(|a| a.ema_latency_ms)
                    .sum::<f32>()
                    / discovery.agents.len() as f32,
            )
        };
        (discovery.agents.len(), busy, total, ema)
    } else {
        let busy: u32 = agents.list.iter().map(|a| a.slots_busy).sum();
        let total: u32 = agents.list.iter().map(|a| a.slots_total).sum();
        let ema = Some(
            agents.list.iter().map(|a| a.ema_latency_ms).sum::<f32>() / agents.list.len() as f32,
        );
        (agents.list.len(), busy, total, ema)
    };
    let (status, status_color) = if agents.list.is_empty() {
        if discovery.agents.is_empty() {
            (String::from("Aguardando"), theme::STALE)
        } else {
            (String::from("Via DDS"), theme::OK)
        }
    } else {
        (format!("{count} Online"), theme::OK)
    };
    TeleCard {
        dot,
        title: String::from("Agentes (HTTP :8080)"),
        status,
        status_color,
        subtitle: String::from("Orquestrador de Despacho & Agendamento"),
        rows: vec![
            (
                String::from("Alocação de Slots:"),
                if count == 0 {
                    String::from("—")
                } else {
                    format!("{busy}/{total} ocupados")
                },
                if count == 0 { theme::STALE } else { cyan },
            ),
            (
                String::from("Latência EMA:"),
                ema_ms
                    .map(|ema| format!("{ema:.0}ms"))
                    .unwrap_or_else(|| String::from("—")),
                if ema_ms.is_some() { cyan } else { theme::STALE },
            ),
            (
                String::from("Via DDS:"),
                format!("{} agente(s)", discovery.agents.len()),
                cyan,
            ),
        ],
        footer: (
            String::from("Origem:"),
            if agents.url.is_empty() {
                String::from("—")
            } else {
                agents.url.chars().take(30).collect()
            },
        ),
    }
}
