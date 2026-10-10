//! Painel 3.10 Máquinas / Descoberta (REQ/T-840-03 + T-890-03): fiel ao
//! mockup Stitch atual — hero com participantes/varredura/"Forçar probe
//! multicast", tabela canonical das instalações descobertas via
//! `Studio.NodePresence`, janela de lease, classificador de falha, mini
//! topologia do domínio, registro manual (Testar & Registrar) e catálogo
//! compartilhado persistido.
//!
//! Só dados reais: sem GUID/latência/ops inventados. O token digitado fica
//! SÓ na memória da GUI (RNF-04 / SDD §28): nunca é enviado ao catálogo,
//! persistido ou impresso. Probe 401 (nó conforme T-840-01) vira badge
//! roxo "falha de auth" — distinto de offline.

use crate::discovery::{DiscoveredNode, DiscoveryState};
use crate::kit;
use crate::machines::{now_unix_ns, MachinesState, ProbeState};
use crate::theme;
use eframe::egui;

/// Lease do tópico `Studio.NodePresence` (s) — barras da janela de lease.
const LEASE_SECS: f32 = 10.0;
/// Período do heartbeat de presença (s) — ciclo de varredura do hero.
const HEARTBEAT_SECS: u64 = 5;

/// Cor do ponto de status por estado da sonda (ciano = autenticado, roxo =
/// falha de auth, vermelho = offline — legenda do mockup).
fn dot_color(state: ProbeState) -> egui::Color32 {
    match state {
        ProbeState::Online => theme::PRIMARY_FIXED_DIM,
        ProbeState::Offline => theme::ERROR,
        ProbeState::AuthPending => theme::AUTH,
        ProbeState::Unknown => theme::STALE,
    }
}

/// Rótulo do heartbeat pela idade (lease 10 s).
fn heartbeat_label(age_secs: u64) -> (&'static str, egui::Color32) {
    if age_secs <= 5 {
        ("● FRESCO", theme::PRIMARY_FIXED_DIM)
    } else if age_secs <= 10 {
        ("● VIVO", theme::PRIMARY_FIXED_DIM)
    } else {
        ("○ EXPIRANDO", theme::STALE)
    }
}

/// Idade fracionária do último heartbeat (s) — "5.1s atrás" do mockup.
fn age_frac(node: &DiscoveredNode, now_ns: u64) -> f32 {
    now_ns.saturating_sub(node.last_seen_unix_ns) as f32 / 1_000_000_000.0
}

/// Trunca texto com "…" (node ids longos da coluna 1).
fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// "Token recusado" vs "Token ausente" pelo detalhe do probe 401.
fn auth_curto(detail: &str) -> &'static str {
    if detail.contains("recusado") {
        "Token recusado"
    } else {
        "Token ausente"
    }
}

/// Porta da URL (`http://host:porta[/...]`) — linha "porta recusada" honesta.
fn port_of(url: &str) -> Option<&str> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let authority = after_scheme.split('/').next().unwrap_or_default();
    let port = authority.rsplit(':').next().unwrap_or_default();
    (!port.is_empty() && port.chars().all(|c| c.is_ascii_digit())).then_some(port)
}

/// Moldura dos cards da 3.10 (mesmo idioma dos `TeleCard` da 3.1).
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

/// Stat do hero (rótulo + valores): caixa fixa com layout `top_down`
/// explícito — imune à herança de direção do `right_to_left` pai.
fn hero_stat(ui: &mut egui::Ui, width: f32, label: &str, values: impl FnOnce(&mut egui::Ui)) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 48.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .size(9.5)
                    .color(theme::OUTLINE),
            );
            values(ui);
        },
    );
}

pub fn show(ui: &mut egui::Ui, machines: &mut MachinesState, discovery: &mut DiscoveryState) {
    machines.poll();
    discovery.poll();
    // Probes manuais (Re-testar/modal/Testar) voltam em `machines.probes` —
    // espelha para as linhas da descoberta (a cada frame, após o `poll`,
    // para sobreviver à troca do snapshot do worker).
    for node in &mut discovery.nodes {
        let key = crate::machines::normalize_url(&node.url);
        if let Some(status) = machines.probes.get(&key) {
            node.probe = Some(status.clone());
        }
    }
    let domain = discovery.domain;
    let now_ns = now_unix_ns();
    ui.ctx()
        .request_repaint_after(std::time::Duration::from_secs(1));

    // ── Hero (mockup: eyebrow + título + stats + FORÇAR à direita) ──
    let total = discovery.nodes.len();
    let alive = discovery
        .nodes
        .iter()
        .filter(|node| node.is_alive(now_ns))
        .count();
    let freshest_age = discovery
        .nodes
        .iter()
        .map(|node| node.age_secs(now_ns))
        .min();
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("SEÇÃO 3.10 · DESCOBERTA AUTOMÁTICA")
                        .monospace()
                        .size(10.5)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                kit::badge(ui, "TOPIC: Studio.NodePresence", theme::OUTLINE);
                kit::badge(
                    ui,
                    &format!("DOMÍNIO DDS {domain}"),
                    theme::PRIMARY_FIXED_DIM,
                );
            });
            ui.label(
                egui::RichText::new(format!("Descoberta de Nós no Domínio DDS {domain}"))
                    .size(22.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●")
                        .size(12.0)
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                // Âncora kittest: "Escutando tráfego Studio.NodePresence".
                ui.label(
                    egui::RichText::new(
                        "Escutando tráfego Studio.NodePresence · Lease 10s · Poda 30s · \
                         Heartbeat a cada 5s",
                    )
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
            });
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!machines.busy && total > 0, |ui| {
                let force = ui.button(
                    egui::RichText::new("FORÇAR PROBE MULTICAST")
                        .monospace()
                        .strong(),
                );
                if force.clicked() {
                    let urls: Vec<String> = discovery.nodes.iter().map(|n| n.url.clone()).collect();
                    machines.probe_all(&urls);
                    crate::studio_log::info(format!(
                        "máquinas: probe forçado de {} nó(s) pelo usuário",
                        urls.len()
                    ));
                }
            });
            // Stats em caixas FIXAS com layout explícito: `vertical` dentro
            // de horizontal `right_to_left` reivindica toda a largura e os
            // irmãos seguintes colapsam (rótulos saem na vertical).
            hero_stat(ui, 150.0, "CICLO DE VARREDURA", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{HEARTBEAT_SECS}.0s"))
                            .size(17.0)
                            .strong()
                            .color(theme::ON_SURFACE),
                    );
                    match freshest_age {
                        Some(age) => {
                            let next = HEARTBEAT_SECS.saturating_sub(age);
                            ui.label(
                                egui::RichText::new(format!("próx: {next}.0s"))
                                    .monospace()
                                    .small()
                                    .color(theme::PRIMARY_FIXED_DIM),
                            );
                        }
                        None => {
                            ui.label(egui::RichText::new("próx: —").monospace().small().weak());
                        }
                    }
                });
            });
            hero_stat(ui, 165.0, "PARTICIPANTES ATIVOS", |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("{alive}/ "))
                            .size(17.0)
                            .strong()
                            .color(theme::PRIMARY_FIXED_DIM),
                    );
                    ui.label(
                        egui::RichText::new(format!("{total} nós"))
                            .size(14.0)
                            .color(theme::ON_SURFACE),
                    );
                });
            });
        });
    });
    ui.add_space(theme::SPACE_SM);

    if discovery.scanning && discovery.nodes.is_empty() {
        kit::empty_state(ui, "escutando o domínio…");
    }
    if !discovery.error.is_empty() {
        kit::error_banner(ui, &discovery.error);
    }

    // ── Título da seção + legenda (mockup: legenda à direita) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("INSTALAÇÕES DESCOBERTAS NO BARRAMENTO")
                .size(15.0)
                .strong()
                .color(theme::ON_SURFACE),
        );
        kit::badge(ui, "RTPS MULTICAST 239.255.0.1", theme::PRIMARY_FIXED_DIM);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            for (text, color) in [
                ("Timeout / Porta Recusada", theme::ERROR),
                ("Falha Auth (401)", theme::AUTH),
                ("Autenticado", theme::PRIMARY_FIXED_DIM),
            ] {
                ui.label(egui::RichText::new(text).small().color(color));
                ui.label(egui::RichText::new("●").small().color(color));
            }
        });
    });
    ui.add_space(theme::SPACE_XS);

    // ── Tabela canonical das instalações (mockup 3.10) ──
    if discovery.nodes.is_empty() {
        kit::empty_state(
            ui,
            &format!(
                "Nenhuma instalação vista ainda no domínio {domain} (heartbeats a \
                 cada 5 s; nós com canvas DDS aparecem sozinhos aqui)."
            ),
        );
    } else {
        kit::table("discovery_grid").show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "Estado / Node ID",
                    "Ponto de Extremidade (URL)",
                    "Probe de Protocolo & Versão",
                    "Autenticação (Bearer)",
                    "Heartbeat & Ações",
                ],
            );
            for (index, node) in discovery.nodes.iter().enumerate() {
                show_discovery_row(ui, machines, &mut discovery.selected, index, node, now_ns);
                ui.end_row();
            }
        });
    }
    ui.add_space(theme::SPACE_LG);

    // ── Linha de 3 cards: lease · classificador · mini-topologia ──
    ui.columns(3, |cols| {
        cols[0].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "JANELA DE LEASE (10S)", "Poda após 30s silêncio");
                ui.add_space(theme::SPACE_XS);
                if discovery.nodes.is_empty() {
                    ui.label(
                        egui::RichText::new("○ sem instalações no domínio")
                            .small()
                            .weak(),
                    );
                }
                for node in &discovery.nodes {
                    let remaining = (LEASE_SECS - age_frac(node, now_ns)).max(0.0);
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(truncate(&node.node_id, 24))
                                .monospace()
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                            if remaining > 0.0 {
                                ui.label(
                                    egui::RichText::new(format!("{remaining:.1}s restantes"))
                                        .monospace()
                                        .small()
                                        .color(theme::PRIMARY_FIXED_DIM),
                                );
                            } else {
                                ui.label(
                                    egui::RichText::new("Expirado (em poda)")
                                        .monospace()
                                        .small()
                                        .color(theme::ERROR),
                                );
                            }
                        });
                    });
                    let bar_width = ui.available_width();
                    ui.add(
                        egui::ProgressBar::new(if remaining > 0.0 {
                            (remaining / LEASE_SECS).clamp(0.0, 1.0)
                        } else {
                            0.06
                        })
                        .desired_width(bar_width)
                        .fill(if remaining > 0.0 {
                            theme::PRIMARY_FIXED_DIM
                        } else {
                            theme::ERROR
                        }),
                    );
                    ui.add_space(theme::SPACE_XS);
                }
            });
        });
        cols[1].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "CLASSIFICADOR DE FALHA SEMÂNTICA", "SEC-DIAG");
                ui.add_space(theme::SPACE_XS);
                let mut auth_failures = 0usize;
                let mut offline = 0usize;
                for node in &discovery.nodes {
                    if let Some(probe) = &node.probe {
                        match probe.state {
                            ProbeState::AuthPending => auth_failures += 1,
                            ProbeState::Offline => offline += 1,
                            _ => {}
                        }
                    }
                }
                ui.columns(2, |diag| {
                    diag[0].vertical(|ui| {
                        ui.label(
                            egui::RichText::new("● HTTP 401 (Auth)")
                                .small()
                                .strong()
                                .color(theme::AUTH),
                        );
                        ui.label(
                            egui::RichText::new(
                                "Conectividade TCP OK, mas token ausente ou \
                                 inválido no nó remoto.",
                            )
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                    });
                    diag[1].vertical(|ui| {
                        ui.label(
                            egui::RichText::new("● Offline (Rede)")
                                .small()
                                .strong()
                                .color(theme::ERROR),
                        );
                        ui.label(
                            egui::RichText::new(
                                "Socket recusado ou firewall impedindo a porta \
                                 4317.",
                            )
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                    });
                });
                ui.add_space(theme::SPACE_XS);
                ui.label(
                    egui::RichText::new(format!(
                        "agora: {auth_failures} falha(s) de auth · {offline} \
                         offline"
                    ))
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(
                        "*Nenhum segredo ou token é exposto em texto simples no \
                         barramento multicast DDS.",
                    )
                    .small()
                    .weak(),
                );
            });
        });
        cols[2].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(ui, "TOPOLOGIA DO DOMÍNIO", "MESH SPDP/SEDP");
                ui.add_space(theme::SPACE_XS);
                draw_mini_mesh(ui, &discovery.nodes, discovery.selected, now_ns);
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("SPDP Multi: 239.255.0.1:7400")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(format!("DDS DOMAIN {domain}"))
                                .monospace()
                                .small()
                                .strong()
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                    });
                });
            });
        });
    });
    ui.add_space(theme::SPACE_LG);

    // ── Linha de 2 cards: registro manual · catálogo persistido ──
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            show_manual_form(ui, machines);
        });
        cols[1].vertical(|ui| {
            show_catalog(ui, machines);
        });
    });

    // ── Modal: Definir Alvo & Inserir Token ──
    let modal_url = machines.modal_url.clone();
    if let Some(url) = modal_url {
        let mut close = false;
        egui::Window::new("Definir Alvo & Inserir Token")
            .collapsible(false)
            .resizable(false)
            .anchor(egui::Align2::CENTER_CENTER, [0.0, 0.0])
            .show(ui.ctx(), |ui| {
                ui.label(egui::RichText::new("URL DO NÓ:").monospace().small());
                kit::mono_cell(ui, &url);
                ui.add_space(theme::SPACE_SM);
                ui.label(
                    egui::RichText::new("TOKEN BEARER (fica só na memória da GUI):")
                        .monospace()
                        .small(),
                );
                ui.add(
                    egui::TextEdit::singleline(&mut machines.modal_token)
                        .password(true)
                        .desired_width(320.0),
                );
                ui.add_space(theme::SPACE_SM);
                ui.horizontal(|ui| {
                    let confirm = ui.add_enabled(
                        !machines.modal_token.trim().is_empty(),
                        egui::Button::new(
                            egui::RichText::new("Validar & Comutar Alvo").color(theme::ON_PRIMARY),
                        )
                        .fill(theme::PRIMARY_CONTAINER),
                    );
                    if confirm.clicked() {
                        let token = machines.modal_token.trim().to_owned();
                        machines.remember_token(&url, &token);
                        machines.probe_url(&url);
                        // Comuta o alvo único da GUI para este nó.
                        if let Some(index) = discovery.nodes.iter().position(|n| n.url == url) {
                            discovery.selected = Some(index);
                        }
                        crate::studio_log::info(format!(
                            "máquinas: token guardado em memória + alvo comutado para {url}"
                        ));
                        close = true;
                    }
                    if ui.button("Cancelar").clicked() {
                        close = true;
                    }
                });
            });
        if close {
            machines.modal_url = None;
            machines.modal_token.clear();
        }
    }
}

/// Uma linha da tabela de instalações (5 colunas do mockup).
#[allow(clippy::too_many_arguments)]
fn show_discovery_row(
    ui: &mut egui::Ui,
    machines: &mut MachinesState,
    selected: &mut Option<usize>,
    index: usize,
    node: &DiscoveredNode,
    now_ns: u64,
) {
    let alive = node.is_alive(now_ns);
    let age = node.age_secs(now_ns);
    let is_selected = *selected == Some(index);
    let (state_color, state_dot) = match (&node.probe, alive) {
        (Some(probe), _) => (dot_color(probe.state), "●"),
        (None, true) => (theme::WARN, "○"),
        (None, false) => (theme::STALE, "○"),
    };

    // Coluna 1: estado + node id (+ ALVO PRIMÁRIO / ATIVO).
    ui.horizontal(|ui| {
        if is_selected {
            // Filete ciano da linha selecionada (painter — sem glifo ▌).
            let (edge, _) = ui.allocate_exact_size(egui::vec2(3.0, 36.0), egui::Sense::hover());
            ui.painter()
                .rect_filled(edge, 1.0, theme::PRIMARY_FIXED_DIM);
        }
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(egui::RichText::new(state_dot).size(14.0).color(state_color));
                ui.label(
                    egui::RichText::new(truncate(&node.node_id, 22))
                        .size(13.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
            });
            ui.horizontal(|ui| {
                if is_selected {
                    kit::badge(ui, "ALVO PRIMÁRIO", theme::PRIMARY_FIXED_DIM);
                    ui.label(
                        egui::RichText::new(if alive { "● ATIVO" } else { "○ INATIVO" })
                            .monospace()
                            .small()
                            .color(if alive {
                                theme::PRIMARY_FIXED_DIM
                            } else {
                                theme::STALE
                            }),
                    );
                } else {
                    ui.label(
                        egui::RichText::new("○ DESCOBERTO")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                }
            });
        });
    });

    // Coluna 2: URL (sem GUID — o tópico não a expõe; nada inventado).
    ui.vertical(|ui| {
        ui.hyperlink_to(
            egui::RichText::new(&node.url).monospace().small(),
            &node.url,
        );
    });

    // Coluna 3: probe (protocolo ou motivo honesto).
    match &node.probe {
        Some(probe) if probe.state == ProbeState::Online => {
            ui.label(
                egui::RichText::new(format!("✓ {} (OK)", probe.detail))
                    .monospace()
                    .small()
                    .strong()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
        }
        Some(probe) if probe.state == ProbeState::AuthPending => {
            ui.vertical(|ui| {
                let curto = auth_curto(&probe.detail);
                kit::badge(ui, &format!("● HTTP 401 · {curto}"), theme::AUTH);
                ui.label(
                    egui::RichText::new("Canal RPC aberto · Falha de autorização")
                        .monospace()
                        .small()
                        .color(theme::AUTH),
                );
            });
        }
        Some(probe) => {
            ui.vertical(|ui| {
                let refused = probe.detail.to_lowercase().contains("refus");
                match (refused, port_of(&node.url)) {
                    (true, Some(port)) => {
                        kit::badge(
                            ui,
                            &format!("● Offline (porta {port} recusada)"),
                            theme::ERROR,
                        );
                    }
                    _ => {
                        kit::badge(ui, "● Offline", theme::ERROR);
                    }
                }
                ui.label(
                    egui::RichText::new(truncate(&probe.detail, 48))
                        .monospace()
                        .small()
                        .color(theme::ERROR),
                );
            });
        }
        None => {
            ui.label(egui::RichText::new("sondando…").monospace().small().weak());
        }
    }

    // Coluna 4: autenticação (401 é CREDENCIAL, não rede).
    let has_token = machines.token_for(&node.url).is_some();
    match &node.probe {
        Some(probe) if probe.state == ProbeState::Online => {
            ui.vertical(|ui| {
                if node.token_required {
                    if has_token {
                        ui.label(
                            egui::RichText::new("● Bearer configurado")
                                .monospace()
                                .small()
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                        ui.label(egui::RichText::new("(em memória volátil)").small().weak());
                    } else {
                        ui.label(
                            egui::RichText::new("○ sem token em memória")
                                .monospace()
                                .small()
                                .color(theme::WARN),
                        );
                    }
                } else {
                    ui.label(
                        egui::RichText::new("○ canal aberto")
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.label(egui::RichText::new("nó sem auth").small().weak());
                }
            });
        }
        Some(probe) if probe.state == ProbeState::AuthPending => {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new(if has_token {
                        "× Token recusado"
                    } else {
                        "× Não informado"
                    })
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
                );
                ui.label(egui::RichText::new("Requer Bearer Token").small().weak());
            });
        }
        Some(_) => {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("—")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.label(
                    egui::RichText::new("Sem conexão de controle")
                        .small()
                        .weak(),
                );
            });
        }
        None => {
            ui.label(
                egui::RichText::new("—")
                    .monospace()
                    .small()
                    .color(theme::STALE),
            );
        }
    }

    // Coluna 5: heartbeat + ação.
    ui.vertical(|ui| {
        let (hb, hb_color) = heartbeat_label(age);
        ui.label(
            egui::RichText::new(format!("{:.1}s atrás", age_frac(node, now_ns)))
                .monospace()
                .small()
                .color(theme::ON_SURFACE),
        );
        ui.label(
            egui::RichText::new(format!("[{hb}]"))
                .monospace()
                .small()
                .color(hb_color),
        );
        ui.add_space(theme::SPACE_XS);
        if is_selected {
            ui.add_enabled(
                false,
                egui::Button::new(egui::RichText::new("● Alvo Atual").small()),
            );
        } else if matches!(
            &node.probe,
            Some(p) if p.state == ProbeState::AuthPending
        ) {
            let token_btn = kit::primary_button(ui, "Definir Alvo & Inserir Token");
            if token_btn.clicked() {
                machines.modal_url = Some(node.url.clone());
                machines.modal_token.clear();
            }
        } else if matches!(&node.probe, Some(p) if p.state == ProbeState::Online) {
            if ui
                .button(egui::RichText::new("Definir Alvo").small())
                .clicked()
            {
                *selected = Some(index);
                crate::studio_log::info(format!("máquinas: alvo comutado para {}", node.url));
            }
        } else if ui
            .button(egui::RichText::new("↻ Re-testar Conexão").small())
            .clicked()
        {
            machines.probe_url(&node.url);
        }
    });
}

/// Card "Registro Manual no Catálogo" (mockup: 3 campos + Testar & Registrar).
fn show_manual_form(ui: &mut egui::Ui, machines: &mut MachinesState) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Registro Manual no Catálogo")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new("SEÇÃO 5 (PROT)")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
        ui.add_space(theme::SPACE_SM);

        form_field_label(ui, "NOME DO NÓ / ALIAS", "Ex: studio-node-gpu-4090");
        let field_width = ui.available_width();
        ui.add(
            egui::TextEdit::singleline(&mut machines.form_machine_id)
                .desired_width(field_width)
                .hint_text("studio-node-delta-70"),
        );
        ui.add_space(theme::SPACE_SM);
        form_field_label(ui, "URL DO NÓ (RPC TARGET)", "http://IP:4317");
        ui.add(
            egui::TextEdit::singleline(&mut machines.form_node_url)
                .desired_width(field_width)
                .hint_text("http://192.168.1.70:4317"),
        );
        ui.add_space(theme::SPACE_SM);
        form_field_label(ui, "TOKEN / BEARER SECRET", "Guardado apenas na sessão");
        ui.horizontal(|ui| {
            let mut edit = egui::TextEdit::singleline(&mut machines.form_token)
                .hint_text("dss_sec_sk_live_…")
                .desired_width(ui.available_width() - 72.0);
            if !machines.form_token_visible {
                edit = edit.password(true);
            }
            ui.add(edit);
            if ui
                .small_button(if machines.form_token_visible {
                    "Ocultar"
                } else {
                    "Exibir"
                })
                .clicked()
            {
                machines.form_token_visible = !machines.form_token_visible;
            }
        });
        ui.add_space(theme::SPACE_XS);
        ui.label(
            egui::RichText::new(
                "A chave será validada por probe imediato no endpoint /version \
                 antes da persistência.",
            )
            .small()
            .weak(),
        );
        if !machines.notice.is_empty() {
            ui.label(
                egui::RichText::new(&machines.notice)
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
            );
        }
        ui.add_space(theme::SPACE_SM);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.add_enabled_ui(!machines.busy, |ui| {
                let register = kit::primary_button(ui, "Testar & Registrar");
                if register.clicked() {
                    machines.test_and_register();
                }
                if ui.button("Limpar").clicked() {
                    machines.clear_form();
                }
            });
        });
    });
}

/// Rótulo de campo do formulário: legenda à esquerda + dica à direita.
fn form_field_label(ui: &mut egui::Ui, label: &str, hint: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(label)
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new(hint).small().color(theme::OUTLINE));
        });
    });
}

/// Card "Catálogo Compartilhado (Persistido)" (mockup: REV + tabela + rodapé).
fn show_catalog(ui: &mut egui::Ui, machines: &mut MachinesState) {
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Catálogo Compartilhado (Persistido)")
                    .size(14.0)
                    .strong()
                    .color(theme::ON_SURFACE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.add_enabled_ui(!machines.busy, |ui| {
                    if ui.small_button("↻").clicked() {
                        machines.refresh();
                    }
                });
                ui.label(
                    egui::RichText::new(format!("REV: {}", machines.cursor))
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
            });
        });
        ui.add_space(theme::SPACE_SM);

        let list = machines.machines();
        if list.is_empty() {
            kit::empty_state(
                ui,
                "Nenhuma máquina no snapshot do alvo — atualize ou registre ao lado.",
            );
        } else {
            kit::table("machines_grid").show(ui, |ui| {
                kit::grid_header(
                    ui,
                    &[
                        "Identificador Persistido",
                        "URL Base / Porta",
                        "Modo Auth",
                        "Comandos",
                    ],
                );
                for machine in &list {
                    let (dot, color) = match machines.probes.get(&machine.node_url) {
                        Some(status) => (dot_of(status.state), dot_color(status.state)),
                        None => ("●", theme::STALE),
                    };
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(dot).small().color(color));
                        kit::mono_cell(ui, &machine.machine_id);
                    });
                    kit::mono_cell(ui, &machine.node_url);
                    match machines.probes.get(&machine.node_url) {
                        None => kit::badge(ui, "SEM_RESPOSTA", theme::STALE),
                        Some(status) => match status.state {
                            ProbeState::Online => kit::badge(ui, "TOKEN_OK", theme::OK),
                            ProbeState::AuthPending => {
                                kit::badge(ui, "AUTH_PEND (401)", theme::AUTH);
                            }
                            ProbeState::Offline => kit::badge(ui, "OFFLINE", theme::ERROR),
                            ProbeState::Unknown => {
                                kit::badge(ui, "SEM_RESPOSTA", theme::STALE);
                            }
                        },
                    }
                    ui.horizontal(|ui| {
                        ui.add_enabled_ui(!machines.busy, |ui| {
                            if ui.small_button("Probe").clicked() {
                                machines.probe(machine);
                            }
                            if ui.small_button("Token…").clicked() {
                                machines.modal_url = Some(machine.node_url.clone());
                                machines.modal_token.clear();
                            }
                        });
                    });
                    ui.end_row();
                }
            });
        }
        ui.add_space(theme::SPACE_XS);
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("Arquivo de Persistência: ~/.dds_studio/nodes_catalog.json")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                ui.label(
                    egui::RichText::new(format!("{} nós configurados", list.len()))
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
            });
        });
    });
}

/// Glifo do ponto por estado da sonda.
fn dot_of(state: ProbeState) -> &'static str {
    match state {
        ProbeState::Online | ProbeState::AuthPending => "●",
        ProbeState::Offline | ProbeState::Unknown => "○",
    }
}

/// Mini-mapa mesh do domínio (card da 3.10): pontos coloridos por estado com
/// elos SPDP/SEDP pontilhados — esquemático (posições em elipse, não dados).
fn draw_mini_mesh(
    ui: &mut egui::Ui,
    nodes: &[DiscoveredNode],
    selected: Option<usize>,
    now_ns: u64,
) {
    let height = 110.0;
    let (rect, _) = ui.allocate_exact_size(
        egui::vec2(ui.available_width(), height),
        egui::Sense::hover(),
    );
    let painter = ui.painter();
    if nodes.is_empty() {
        painter.text(
            rect.center(),
            egui::Align2::CENTER_CENTER,
            "○ sem instalações no domínio",
            egui::FontId::proportional(11.0),
            theme::OUTLINE,
        );
        return;
    }
    let shown = nodes.len().min(12);
    let center = rect.center();
    let radius = egui::vec2(rect.width() / 2.0 - 16.0, rect.height() / 2.0 - 12.0);
    let positions: Vec<egui::Pos2> = (0..shown)
        .map(|i| {
            let angle =
                -std::f32::consts::FRAC_PI_2 + i as f32 / shown as f32 * std::f32::consts::TAU;
            center + egui::vec2(angle.cos() * radius.x, angle.sin() * radius.y)
        })
        .collect();
    for i in 0..shown {
        for j in (i + 1)..shown {
            dashed_line(painter, positions[i], positions[j], theme::OUTLINE_VARIANT);
        }
    }
    for (i, pos) in positions.iter().enumerate() {
        let node = &nodes[i];
        let alive = node.is_alive(now_ns);
        let color = match (&node.probe, alive) {
            (Some(probe), _) => dot_color(probe.state),
            (None, true) => theme::PRIMARY_FIXED_DIM,
            (None, false) => theme::STALE,
        };
        if alive {
            painter.circle_stroke(*pos, 8.0, egui::Stroke::new(1.5, color));
        }
        painter.circle_filled(*pos, 4.0, color);
        if selected == Some(i) {
            painter.circle_stroke(*pos, 11.5, egui::Stroke::new(1.0, theme::ON_SURFACE));
        }
    }
    if nodes.len() > shown {
        painter.text(
            rect.right_bottom() + egui::vec2(-4.0, -2.0),
            egui::Align2::RIGHT_BOTTOM,
            format!("+{}", nodes.len() - shown),
            egui::FontId::monospace(10.0),
            theme::OUTLINE,
        );
    }
}

/// Segmento tracejado (elo SPDP/SEDP do mini-mapa).
fn dashed_line(painter: &egui::Painter, from: egui::Pos2, to: egui::Pos2, color: egui::Color32) {
    let len = from.distance(to);
    if len < 1.0 {
        return;
    }
    let dir = (to - from).normalized();
    let (dash, gap) = (5.0, 4.0);
    let mut travelled = 0.0;
    while travelled < len {
        let end = (travelled + dash).min(len);
        painter.line_segment(
            [from + dir * travelled, from + dir * end],
            egui::Stroke::new(1.0, color),
        );
        travelled += dash + gap;
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn auth_curto_distinguishes_refused_from_missing() {
        assert_eq!(auth_curto("token recusado (401)"), "Token recusado");
        assert_eq!(
            auth_curto("token ausente (nó respondeu 401)"),
            "Token ausente"
        );
    }

    #[test]
    fn port_of_parses_url_port() {
        assert_eq!(port_of("http://192.168.1.61:4317"), Some("4317"));
        assert_eq!(port_of("http://lab-01:4317/rpc"), Some("4317"));
        assert_eq!(port_of("http://lab-01"), None);
        assert_eq!(port_of("http://[fe80::1]:4317"), Some("4317"));
    }

    #[test]
    fn truncate_keeps_short_and_ellipsizes_long() {
        assert_eq!(truncate("abc", 22), "abc");
        assert_eq!(truncate("studio-node-alpha-61-extra", 10), "studio-no…");
    }

    #[test]
    fn heartbeat_label_thresholds_follow_lease() {
        assert_eq!(heartbeat_label(0).0, "● FRESCO");
        assert_eq!(heartbeat_label(6).0, "● VIVO");
        assert_eq!(heartbeat_label(11).0, "○ EXPIRANDO");
    }

    #[test]
    fn age_frac_counts_seconds_from_last_seen() {
        let node = DiscoveredNode {
            node_id: String::from("n"),
            url: String::from("http://n:4317"),
            token_required: false,
            last_seen_unix_ns: 1_000_000_000,
            probe: None,
        };
        assert!((age_frac(&node, 6_500_000_000) - 5.5).abs() < 0.001);
    }
}
