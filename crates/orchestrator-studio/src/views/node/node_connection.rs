//! Parte de `views::node` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_connection(
    ui: &mut egui::Ui,
    state: &mut AppState,
    node_url: &mut String,
    node_token: &mut String,
    discovery: &DiscoveryState,
) {
    let status = state.status().to_owned();
    let connected = state.node().is_some();
    let busy = state.busy();
    let (_live_label, live_color) = live_state(&status, connected, busy);
    let protocol = state
        .node()
        .map(|node| format!("{}.{}", node.version.major, node.version.minor))
        .unwrap_or_else(|| String::from("—"));
    // ── Card de conexão RPC + autenticação in-memory (mockup) ──
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        card_title(
            ui,
            "INTERFACE DE CONEXÃO RPC & AUTENTICAÇÃO IN-MEMORY",
            "● LOOP_INTERVAL: 1000ms",
        );
        ui.add_space(theme::SPACE_XS);
        ui.columns(3, |cols| {
            cols[0].vertical(|ui| {
                // Cabeçalho em 2 linhas (1500px não comporta rótulo +
                // keepalive lado a lado sem invadir a coluna vizinha).
                ui.label(
                    egui::RichText::new("PONTO DE EXTREMIDADE (URL DO NÓ)")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.label(
                    egui::RichText::new("TCP_KEEPALIVE: ATIVO")
                        .monospace()
                        .small()
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                let field_w = ui.available_width();
                ui.add(
                    egui::TextEdit::singleline(node_url)
                        .desired_width(field_w)
                        .hint_text("http://IP:4317"),
                );
                if let Some(node) = discovery.nodes.iter().find(|n| n.url == *node_url) {
                    ui.label(
                        egui::RichText::new(format!("(id na descoberta: {})", node.node_id))
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                } else if !node_url.trim().is_empty() {
                    ui.label(
                        egui::RichText::new("URL manual (fora da descoberta)")
                            .small()
                            .weak(),
                    );
                }
            });
            cols[1].vertical(|ui| {
                // 2 linhas (mesmo motivo da col. 0: sem invasão lateral).
                ui.label(
                    egui::RichText::new("TOKEN BEARER (SESSÃO TRANSITÓRIA)")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.label(
                    egui::RichText::new("Em memória volátil (Zero disco / Zero persistência)")
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.horizontal(|ui| {
                    let mut visible: bool = ui
                        .ctx()
                        .data(|data| data.get_temp(egui::Id::new("node-token-visible")))
                        .unwrap_or(false);
                    let mut edit = egui::TextEdit::singleline(node_token)
                        .desired_width(ui.available_width() - 72.0)
                        .hint_text("dss_sec_…");
                    if !visible {
                        edit = edit.password(true);
                    }
                    ui.add(edit);
                    if ui
                        .small_button(if visible { "Ocultar" } else { "Exibir" })
                        .clicked()
                    {
                        visible = !visible;
                    }
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(egui::Id::new("node-token-visible"), visible)
                    });
                });
            });
            cols[2].vertical(|ui| {
                ui.label(
                    egui::RichText::new("COMANDO DE SONDA")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                // Linha em branco: alinha os botões aos campos das cols. 0–1
                // (cabeçalhos de 2 linhas).
                ui.label(egui::RichText::new(" ").small());
                ui.horizontal(|ui| {
                    ui.add_enabled_ui(!busy, |ui| {
                        let token = optional_token(node_token).map(str::to_string);
                        if ui.button("Atualizar").clicked() {
                            state.refresh_from_node_with_token(node_url, token.as_deref());
                            crate::studio_log::info(format!(
                                "nó: atualização manual de {node_url}"
                            ));
                        }
                        let reconnect = kit::primary_button(ui, "Re-conectar / Forçar Probe");
                        if reconnect.clicked() {
                            state.refresh_from_node_with_token(node_url, token.as_deref());
                            crate::studio_log::info(format!("nó: reconexão manual a {node_url}"));
                        }
                    });
                });
            });
        });
        ui.add_space(theme::SPACE_XS);
        // Linha de status: estado · protocolo · RTT medido · socket · modo.
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(match (connected, busy) {
                    (true, _) => "● Conectado",
                    (false, true) => "○ Conectando…",
                    (false, false) if status.contains("negado") => "● Acesso negado (401)",
                    (false, false) if status != "sem leitura do nó" => "○ Desconectado",
                    (false, false) => "○ Sem leitura",
                })
                .small()
                .strong()
                .color(live_color),
            );
            ui.label(
                egui::RichText::new(format!("Protocolo: v{protocol} (OK)"))
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
            );
            ui.label(egui::RichText::new("|").small().weak());
            match state
                .rtt_ms()
                .or_else(|| state.rtt_ema_ms().map(|ema| ema as u64))
            {
                Some(rtt) => {
                    ui.label(
                        egui::RichText::new("Latência Round-trip:")
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.label(
                        egui::RichText::new(format!("{rtt}ms"))
                            .monospace()
                            .small()
                            .strong()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                }
                None => {
                    ui.label(
                        egui::RichText::new("Latência Round-trip: —")
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                }
            }
            ui.label(egui::RichText::new("|").small().weak());
            ui.label(
                egui::RichText::new(format!(
                    "Socket: {}",
                    if connected { "ESTABLISHED" } else { "—" }
                ))
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                let has_token = !node_token.trim().is_empty();
                ui.label(
                    egui::RichText::new(if has_token {
                        "MODO: LER_ESCREVER (AUTORIZADO PELA SESSÃO)"
                    } else {
                        "MODO: LEITURA (SEM TOKEN)"
                    })
                    .monospace()
                    .small()
                    .color(if has_token {
                        theme::PRIMARY_FIXED_DIM
                    } else {
                        theme::WARN
                    }),
                );
            });
        });
    });
    ui.add_space(theme::SPACE_LG);
}
