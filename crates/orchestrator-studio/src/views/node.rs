//! Painel 3.2 Nó studio-node: interface de conexão com o alvo único da
//! descoberta (select de endpoints), token em memória volátil, tríade de
//! métricas (protocolo · operações · conexão), log de operações com inspetor
//! e card de falha 401×rede.

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::origin::optional_token;
use crate::panel_header::panel_header;
use crate::state::AppState;
use crate::theme;
use eframe::egui;
use studio_node::protocol::AdminOp;

/// Resumo de uma linha para a tabela de operações do nó.
fn op_summary(op: &AdminOp) -> String {
    match op {
        AdminOp::Bootstrap { node_name } => format!("bootstrap {node_name}"),
        AdminOp::SetService { service, running } => {
            format!("serviço {service} {}", if *running { "on" } else { "off" })
        }
    }
}

/// Tipo da operação (coluna da tabela).
fn op_kind(op: &AdminOp) -> &'static str {
    match op {
        AdminOp::Bootstrap { .. } => "NODE_BOOTSTRAP",
        AdminOp::SetService { .. } => "SERVICE_ACTION",
    }
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    node_url: &mut String,
    node_token: &mut String,
    discovery: &DiscoveryState,
) {
    panel_header(
        ui,
        "SEC 3.2 · NÓ STUDIO-NODE · PORTA 4317",
        "Nó studio-node",
        "Daemon local por máquina · protocolo RPC + log de operações · token \
         Bearer em memória volátil (zero disco / zero persistência)",
    );
    // Drena o worker de leitura do nó (thread + mpsc — REQ/T-820-19).
    state.poll();

    // ── Interface de conexão: endpoints da descoberta + token em memória ──
    kit::section_label(ui, "INTERFACE DE CONEXÃO RPC & AUTENTICAÇÃO EM MEMÓRIA");
    ui.horizontal(|ui| {
        ui.label("Ponto de Extremidade (URL do nó):");
        egui::ComboBox::from_id_salt("node-endpoint-select")
            .selected_text(if node_url.is_empty() {
                "— escolher da descoberta —".to_owned()
            } else {
                node_url.clone()
            })
            .show_ui(ui, |ui| {
                for node in &discovery.nodes {
                    ui.selectable_value(node_url, node.url.clone(), node.url.clone());
                }
            });
        ui.label("ou digite:");
        ui.text_edit_singleline(node_url);
    });
    ui.horizontal(|ui| {
        ui.label("Token Bearer (sessão transitória):");
        ui.add(egui::TextEdit::singleline(node_token).password(true));
        ui.label(
            egui::RichText::new("· vive só na memória da GUI")
                .small()
                .weak(),
        );
        ui.add_enabled_ui(!state.busy(), |ui| {
            if ui
                .add(egui::Button::new(
                    egui::RichText::new("Re-conectar / Forçar Probe")
                        .monospace()
                        .small()
                        .color(theme::ON_PRIMARY),
                ))
                .clicked()
            {
                let url = node_url.clone();
                state.refresh_from_node_with_token(&url, optional_token(node_token));
                crate::studio_log::info(format!("nó: reconexão manual a {url}"));
            }
        });
    });
    ui.add_space(theme::SPACE_MD);

    // ── Tríade de métricas (protocolo · operações · conexão) ──
    let status = state.status().to_owned();
    let (protocol, ops_count) = state
        .node()
        .map(|node| {
            (
                format!("{}.{}", node.version.major, node.version.minor),
                node.operations.len().to_string(),
            )
        })
        .unwrap_or_else(|| (String::from("—"), String::from("—")));
    let connection_ok = state.node().is_some();
    ui.columns(3, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Protocolo do binário",
            protocol,
            "GET /version (compatibilidade §31)",
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Operações registradas",
            ops_count,
            "GET /operations (log do nó)",
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[2],
            "Conexão",
            if connection_ok {
                "CONECTADO".to_owned()
            } else if state.busy() {
                "CONECTANDO…".to_owned()
            } else {
                "SEM LEITURA".to_owned()
            },
            &status,
            if connection_ok {
                theme::OK
            } else if state.busy() {
                theme::WARN
            } else {
                theme::STALE
            },
        );
    });
    ui.add_space(theme::SPACE_MD);

    // ── Card de falha: 401 (credencial) × rede (distinctos por design) ──
    if !connection_ok && !state.busy() && status != "sem leitura do nó" {
        let is_auth = status.contains("negado");
        kit::error_banner(
            ui,
            &format!(
                "{} — {}",
                status,
                if is_auth {
                    "estado de CREDENCIAL: (re)insira o token acima e re-conecte"
                } else {
                    "falha de transporte: confira se o studio-node está ativo na máquina alvo"
                }
            ),
        );
        if is_auth {
            ui.label(
                egui::RichText::new("401 é distinto de offline: o nó está VIVO e exigiu token.")
                    .monospace()
                    .small()
                    .color(theme::AUTH),
            );
        }
        ui.add_space(theme::SPACE_MD);
    }

    // ── Log de operações aplicadas + inspetor ──
    if let Some(node) = state.node() {
        if !node.operations.is_empty() {
            ui.collapsing(
                format!(
                    "Log de Operações Aplicadas · {} registrada(s)",
                    node.operations.len()
                ),
                |ui| {
                    let mut selected: Option<usize> = ui
                        .ctx()
                        .data(|data| data.get_temp(egui::Id::new("node-op-selected")));
                    egui::Grid::new("node_ops_grid")
                        .striped(true)
                        .show(ui, |ui| {
                            kit::grid_header(ui, &["Op ID", "Tipo", "Alvo / contexto"]);
                            for (index, record) in node.operations.iter().enumerate() {
                                let is_selected = selected == Some(index);
                                if ui
                                    .selectable_label(
                                        is_selected,
                                        egui::RichText::new(&record.id.0).monospace(),
                                    )
                                    .clicked()
                                {
                                    selected = Some(index);
                                }
                                ui.label(op_kind(&record.op));
                                ui.label(op_summary(&record.op));
                                ui.end_row();
                            }
                        });
                    ui.ctx().data_mut(|data| {
                        data.insert_temp(egui::Id::new("node-op-selected"), selected)
                    });
                    if let Some(index) = selected {
                        if let Some(record) = node.operations.get(index) {
                            ui.add_space(theme::SPACE_SM);
                            kit::section_label(ui, "INSPETOR DE OPERAÇÃO RPC");
                            egui::Frame::NONE
                                .fill(theme::SURFACE_LOW)
                                .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
                                .inner_margin(theme::SPACE_MD)
                                .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
                                .show(ui, |ui| {
                                    ui.set_min_width(ui.available_width());
                                    ui.label(
                                        egui::RichText::new(format!(
                                            "operation_id: {}",
                                            record.id.0
                                        ))
                                        .monospace()
                                        .small()
                                        .color(theme::PRIMARY_FIXED_DIM),
                                    );
                                    ui.label(
                                        egui::RichText::new(format!("{:#?}", record.op))
                                            .monospace()
                                            .small(),
                                    );
                                });
                        }
                    }
                },
            );
        }
    }
}
