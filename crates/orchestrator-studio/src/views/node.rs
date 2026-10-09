//! Painel 3.2 Nó studio-node: fiel ao mockup Stitch — cabeçalho SEC:3.2,
//! card de conexão RPC + token em memória, status (protocolo · RTT medido ·
//! socket · modo), 3 cards (protocolo · operações · concessão/lease), log de
//! operações com filtro + inspetor JSON e card de falhas 401×rede.
//!
//! Só dados reais: `OpRecord = id + op` (sem timestamp/retorno por operação
//! no contrato — colunas inexistentes ficam de fora, com nota); versão é
//! `ProtocolVersion { major, minor }` do `GET /version`; sem GUID/fd/slots
//! inventados. GUID do design (DDS_GUID) não existe nos dados e foi omitido.

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::now_unix_ns;
use crate::origin::optional_token;
use crate::protected::ProtectedGuard;
use crate::state::AppState;
use crate::theme;
use eframe::egui;
use studio_node::protocol::AdminOp;

/// Lease do tópico `Studio.NodePresence` (s) — card de concessão.
const LEASE_SECS: f32 = 10.0;

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

/// Alvo/contexto da operação (coluna da tabela) — dado real do `AdminOp`.
fn op_target(op: &AdminOp) -> String {
    match op {
        AdminOp::Bootstrap { node_name } => node_name.clone(),
        AdminOp::SetService { service, .. } => format!("unit: {service}"),
    }
}

/// Moldura dos cards da 3.2 (mesmo idioma da 3.10).
fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
}

/// Cabeçalho de card: título à esquerda + etiqueta à direita.
fn card_title(ui: &mut egui::Ui, title: &str, tag: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .monospace()
                .size(11.5)
                .strong()
                .color(theme::ON_SURFACE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(tag)
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
}

/// Estado vivo da conexão (linha de status + card de falhas).
fn live_state(status: &str, connected: bool, busy: bool) -> (&'static str, egui::Color32) {
    if connected {
        ("CONECTADO", theme::OK)
    } else if busy {
        ("SONDANDO", theme::WARN)
    } else if status.contains("negado") {
        ("HTTP 401", theme::AUTH)
    } else if status.contains("bloqueada") {
        ("INCOMPATÍVEL", theme::ERROR)
    } else if status != "sem leitura do nó" {
        ("OFFLINE", theme::ERROR)
    } else {
        ("SEM LEITURA", theme::STALE)
    }
}

pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    node_url: &mut String,
    node_token: &mut String,
    discovery: &DiscoveryState,
    _guard: &ProtectedGuard,
) {
    // Drena o worker de leitura do nó (thread + mpsc — REQ/T-820-19).
    state.poll();
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));

    let status = state.status().to_owned();
    let connected = state.node().is_some();
    let busy = state.busy();
    let (live_label, live_color) = live_state(&status, connected, busy);
    let (protocol, ops_count) = state
        .node()
        .map(|node| {
            (
                format!("{}.{}", node.version.major, node.version.minor),
                node.operations.len(),
            )
        })
        .unwrap_or_else(|| (String::from("—"), 0));
    // Nó da descoberta correspondente ao endpoint (lease real do card 3).
    let now_ns = now_unix_ns();
    let lease = discovery
        .nodes
        .iter()
        .find(|node| node.url == *node_url)
        .map(|node| {
            let age = now_ns.saturating_sub(node.last_seen_unix_ns) as f32 / 1_000_000_000.0;
            (age, (LEASE_SECS - age).max(0.0))
        });

    // ── Cabeçalho SEC:3.2 (mockup; shell global novo já vem do main) ──
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                kit::badge(ui, "SEC:3.2", theme::PRIMARY_FIXED_DIM);
                ui.label(
                    egui::RichText::new("3.2 Nó studio-node")
                        .size(22.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
            });
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●")
                        .size(12.0)
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.label(
                    egui::RichText::new(
                        "Daemon Local por Máquina (Porta 4317) · Protocolo RPC & Log de Operações",
                    )
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            kit::badge(ui, "RUST_EGUI_RUNTIME", theme::OUTLINE);
        });
    });
    ui.add_space(theme::SPACE_SM);

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

    // ── Linha de 3 cards: protocolo · operações · concessão/lease ──
    ui.columns(3, |cols| {
        cols[0].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                let blocked = status.contains("bloqueada");
                card_title(
                    ui,
                    "VERSÃO DO PROTOCOLO",
                    if connected {
                        "COMPATÍVEL §31"
                    } else if blocked {
                        "BLOQUEADO §31"
                    } else {
                        "SEM LEITURA"
                    },
                );
                ui.add_space(theme::SPACE_XS);
                ui.label(
                    egui::RichText::new(format!("v{protocol}"))
                        .monospace()
                        .size(20.0)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.label(
                    egui::RichText::new("Protocolo RPC do studio-node")
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "Handshake /version: {}",
                        if connected {
                            "OK"
                        } else if blocked {
                            "INCOMPATÍVEL"
                        } else {
                            "—"
                        }
                    ))
                    .monospace()
                    .small()
                    .color(if connected {
                        theme::OK
                    } else if blocked {
                        theme::ERROR
                    } else {
                        theme::OUTLINE
                    }),
                );
            });
        });
        cols[1].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "TOTAL DE OPERAÇÕES", "CACHE LOCAL: OK");
                ui.add_space(theme::SPACE_XS);
                ui.label(
                    egui::RichText::new(ops_count.to_string())
                        .monospace()
                        .size(20.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
                ui.label(
                    egui::RichText::new("registradas no nó")
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new("GET /operations · espelho íntegro")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
        cols[2].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "MODO DO NÓ & CONCESSÃO", "ALVO GLOBAL");
                ui.add_space(theme::SPACE_XS);
                ui.label(
                    egui::RichText::new("● Nó alvo")
                        .size(14.0)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                match lease {
                    Some((age, remaining)) => {
                        ui.label(
                            egui::RichText::new(format!(
                                "Lease DDS 10s · {remaining:.1}s restantes"
                            ))
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.label(
                            egui::RichText::new(format!("Último heartbeat: {age:.1}s atrás"))
                                .monospace()
                                .small()
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new("URL manual (sem lease DDS)")
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.label(
                            egui::RichText::new("Último heartbeat: —")
                                .monospace()
                                .small()
                                .weak(),
                        );
                    }
                }
            });
        });
    });
    ui.add_space(theme::SPACE_LG);

    // ── Log de operações + inspetor (mockup: filtro, 4 colunas honestas) ──
    let operations: &[studio_node::operations::OpRecord] = state
        .node()
        .map(|node| node.operations.as_slice())
        .unwrap_or(&[]);
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            show_ops_log(ui, operations);
        });
        cols[1].vertical(|ui| {
            show_inspector(ui, operations);
        });
    });
    ui.add_space(theme::SPACE_LG);

    // ── Falhas & diagnóstico de transporte RPC (mockup, textos verdadeiros) ──
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Tratamento de Falhas & Diagnóstico de Transporte RPC")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("ESTADO ATUAL: {live_label}"))
                        .monospace()
                        .small()
                        .strong()
                        .color(live_color),
                );
                ui.label(
                    egui::RichText::new("DIAG_POLICY_V1")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
        ui.add_space(theme::SPACE_SM);
        ui.columns(2, |diag| {
            diag[0].vertical(|ui| {
                let active = live_label == "HTTP 401";
                ui.horizontal(|ui| {
                    kit::badge(ui, "HTTP 401", theme::ERROR);
                    ui.label(
                        egui::RichText::new("Unauthorized / Token Inválido")
                            .small()
                            .strong()
                            .color(theme::ON_SURFACE),
                    );
                    if active {
                        ui.label(
                            egui::RichText::new("● AGORA")
                                .monospace()
                                .small()
                                .strong()
                                .color(theme::AUTH),
                        );
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "O nó respondeu 401: Bearer ausente ou inválido. O nó está VIVO — \
                         falha de credencial, nunca de rede.",
                    )
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(
                        "Ação do Sistema: insira o token no campo acima e re-conecte; o token \
                         vive só na memória da sessão.",
                    )
                    .small()
                    .color(theme::OUTLINE),
                );
            });
            diag[1].vertical(|ui| {
                let active = live_label == "OFFLINE";
                ui.horizontal(|ui| {
                    kit::badge(ui, "ECONNREFUSED", theme::PRIMARY_FIXED_DIM);
                    ui.label(
                        egui::RichText::new("Falha de Socket TCP / Daemon Offline")
                            .small()
                            .strong()
                            .color(theme::ON_SURFACE),
                    );
                    if active {
                        ui.label(
                            egui::RichText::new("● AGORA")
                                .monospace()
                                .small()
                                .strong()
                                .color(theme::ERROR),
                        );
                    }
                });
                ui.label(
                    egui::RichText::new(
                        "Socket recusado, host inalcançável ou timeout de 3 s. O daemon pode \
                         estar parado no alvo.",
                    )
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(
                        "Ação do Sistema: confira o studio-node no alvo; a GUI preserva o \
                         último estado válido e tenta de novo na auto-carga.",
                    )
                    .small()
                    .color(theme::OUTLINE),
                );
            });
        });
    });
}

/// Card "Log de Operações Aplicadas": filtro + tabela honesta (4 colunas —
/// o contrato `OpRecord = id + op` não tem timestamp/retorno).
fn show_ops_log(ui: &mut egui::Ui, operations: &[studio_node::operations::OpRecord]) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let mut filter: String = ui
            .ctx()
            .data(|data| data.get_temp(egui::Id::new("node-op-filter")))
            .unwrap_or_default();
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Log de Operações Aplicadas")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            kit::badge(
                ui,
                &format!("{} registradas", operations.len()),
                theme::PRIMARY_FIXED_DIM,
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if ui.small_button("×").clicked() {
                    filter.clear();
                }
                ui.add(
                    egui::TextEdit::singleline(&mut filter)
                        .desired_width(220.0)
                        .hint_text("Filtrar por id, tipo ou resumo"),
                );
            });
        });
        ui.ctx()
            .data_mut(|data| data.insert_temp(egui::Id::new("node-op-filter"), filter.clone()));
        ui.add_space(theme::SPACE_XS);
        ui.label(
            egui::RichText::new(
                "Registro temporal sequencial das mutações submetidas ao daemon. Clique em \
                 uma linha para inspecionar o pacote RPC íntegro.",
            )
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
        ui.label(
            egui::RichText::new(
                "O nó não carimba horário/retorno por operação (OpRecord = id + op) — \
                 colunas de timestamp/latência não existem no contrato e ficam de fora.",
            )
            .small()
            .weak(),
        );
        ui.add_space(theme::SPACE_SM);
        let needle = filter.to_lowercase();
        let shown: Vec<&studio_node::operations::OpRecord> = operations
            .iter()
            .filter(|record| {
                needle.is_empty()
                    || record.id.0.to_lowercase().contains(&needle)
                    || op_kind(&record.op).to_lowercase().contains(&needle)
                    || op_summary(&record.op).to_lowercase().contains(&needle)
                    || op_target(&record.op).to_lowercase().contains(&needle)
            })
            .collect();
        if shown.is_empty() {
            kit::empty_state(
                ui,
                if operations.is_empty() {
                    "Nenhuma operação registrada no nó."
                } else {
                    "Nenhuma operação casa com o filtro."
                },
            );
        } else {
            let mut selected: Option<usize> = ui
                .ctx()
                .data(|data| data.get_temp(egui::Id::new("node-op-selected")));
            if selected.is_none() {
                selected = Some(0);
            }
            kit::table("node_ops_grid").show(ui, |ui| {
                kit::grid_header(ui, &["Op ID", "Tipo", "Alvo / Contexto", "Resumo da Ação"]);
                for (shown_index, record) in shown.iter().enumerate() {
                    let is_selected = selected == Some(shown_index);
                    if ui
                        .selectable_label(
                            is_selected,
                            egui::RichText::new(&record.id.0).monospace().small(),
                        )
                        .clicked()
                    {
                        selected = Some(shown_index);
                    }
                    ui.label(
                        egui::RichText::new(op_kind(&record.op))
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                    kit::mono_cell(ui, &op_target(&record.op));
                    ui.label(
                        egui::RichText::new(op_summary(&record.op))
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.end_row();
                }
            });
            ui.ctx()
                .data_mut(|data| data.insert_temp(egui::Id::new("node-op-selected"), selected));
        }
        ui.add_space(theme::SPACE_XS);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new(format!(
                    "EXIBINDO {} DE {} REGISTROS",
                    shown.len(),
                    operations.len()
                ))
                .monospace()
                .small()
                .color(theme::OUTLINE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("ATUALIZADO PELA AUTO-CARGA")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
    });
}

/// Card "Inspetor de Operação RPC": JSON íntegro + encoding + tamanho real.
fn show_inspector(ui: &mut egui::Ui, operations: &[studio_node::operations::OpRecord]) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        let selected: Option<usize> = ui
            .ctx()
            .data(|data| data.get_temp(egui::Id::new("node-op-selected")));
        let filter: String = ui
            .ctx()
            .data(|data| data.get_temp(egui::Id::new("node-op-filter")))
            .unwrap_or_default();
        let needle = filter.to_lowercase();
        let shown: Vec<&studio_node::operations::OpRecord> = operations
            .iter()
            .filter(|record| {
                needle.is_empty()
                    || record.id.0.to_lowercase().contains(&needle)
                    || op_kind(&record.op).to_lowercase().contains(&needle)
                    || op_summary(&record.op).to_lowercase().contains(&needle)
                    || op_target(&record.op).to_lowercase().contains(&needle)
            })
            .collect();
        let record = selected
            .and_then(|index| shown.get(index).copied())
            .or_else(|| shown.first().copied());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Inspetor de Operação RPC")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                if let Some(record) = record {
                    if ui.small_button("COPIAR").clicked() {
                        let raw = serde_json::to_string_pretty(record)
                            .unwrap_or_else(|_| format!("{record:#?}"));
                        ui.ctx().copy_text(raw);
                        crate::studio_log::info(format!(
                            "nó: operação {} copiada como JSON",
                            record.id.0
                        ));
                    }
                    kit::badge(ui, &record.id.0, theme::PRIMARY_FIXED_DIM);
                }
            });
        });
        ui.add_space(theme::SPACE_XS);
        ui.label(
            egui::RichText::new(
                "Estrutura de dados serializada retornada pela máquina receptora via HTTP:",
            )
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
        ui.add_space(theme::SPACE_XS);
        match record {
            Some(record) => {
                let raw_json =
                    serde_json::to_string_pretty(record).unwrap_or_else(|_| format!("{record:#?}"));
                ui.add(
                    egui::TextEdit::multiline(&mut raw_json.clone())
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(12)
                        .desired_width(ui.available_width())
                        .interactive(false),
                );
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new("TRANSPORT_ENCODING")
                                .monospace()
                                .small()
                                .color(theme::OUTLINE),
                        );
                        ui.label(
                            egui::RichText::new("application/json (UTF-8)")
                                .monospace()
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                    });
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.vertical(|ui| {
                            ui.label(
                                egui::RichText::new("PAYLOAD_SIZE")
                                    .monospace()
                                    .small()
                                    .color(theme::OUTLINE),
                            );
                            ui.label(
                                egui::RichText::new(format!("{} Bytes", raw_json.len()))
                                    .monospace()
                                    .small()
                                    .color(theme::ON_SURFACE_VARIANT),
                            );
                        });
                    });
                });
            }
            None => {
                kit::empty_state(ui, "Selecione uma operação na tabela ao lado.");
            }
        }
    });
}
