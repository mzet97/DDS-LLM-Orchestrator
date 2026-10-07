//! Painel 3.10 Máquinas / Descoberta (REQ/T-840-03 + T-890-03): tabela
//! canonical das instalações descobertas via `Studio.NodePresence`, legenda
//! de autenticação 401×offline, gauges de lease, classificador de falha e o
//! catálogo persistido com badges de modo de autenticação.
//!
//! O token digitado fica SÓ na memória da GUI (RNF-04 / SDD §28): nunca é
//! enviado ao catálogo, persistido ou impresso. Probe 401 (nó conforme
//! T-840-01) vira badge roxo "falha de auth" — distinto de offline.

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::{MachinesState, ProbeState};
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

/// Cor/símbolo do ponto de status por estado da sonda.
fn dot_style(state: ProbeState) -> (egui::Color32, &'static str) {
    match state {
        ProbeState::Online => (theme::OK, state.dot()),
        ProbeState::Offline => (theme::ERROR, state.dot()),
        ProbeState::AuthPending => (theme::AUTH, "🔒"),
        ProbeState::Unknown => (theme::STALE, state.dot()),
    }
}

/// Rótulo do heartbeat pela idade (lease 10 s).
fn heartbeat_label(age_secs: u64) -> (&'static str, egui::Color32) {
    if age_secs <= 5 {
        ("● FRESCO", theme::OK)
    } else if age_secs <= 10 {
        ("● VIVO", theme::OK)
    } else {
        ("◌ EXPIRANDO", theme::STALE)
    }
}

pub fn show(ui: &mut egui::Ui, machines: &mut MachinesState, discovery: &mut DiscoveryState) {
    machines.poll();
    discovery.poll();
    let domain = discovery.domain;

    panel_header(
        ui,
        &format!(
            "SEÇÃO 3.10 · DESCOBERTA AUTOMÁTICA · TÓPICO: STUDIO.NODEPRESENCE · DOMÍNIO DDS {domain}"
        ),
        &format!("Descoberta de Nós no Domínio DDS {domain}"),
        "Escutando tráfego Studio.NodePresence · Lease 10s · Poda 30s · Heartbeat a cada 5s",
    );

    // ── Pills do mockup: participantes + varredura (contagem real) + RTPS ──
    ui.horizontal(|ui| {
        kit::badge(
            ui,
            &format!("PARTICIPANTES ATIVOS: {}", discovery.nodes.len()),
            theme::PRIMARY_FIXED_DIM,
        );
        // Próximo heartbeat esperado: 5 s após o mais fresco do domínio
        // (derivação real — nós publicam presença a cada 5 s).
        let now = crate::machines::now_unix_ns();
        let freshest_age = discovery
            .nodes
            .iter()
            .map(|node| node.age_secs(now))
            .min()
            .unwrap_or(5);
        let next_in = 5_u64.saturating_sub(freshest_age);
        kit::badge(
            ui,
            &format!("CICLO DE VARREDURA · PRÓXIMA ~{next_in}s"),
            theme::ON_SURFACE_VARIANT,
        );
        kit::badge(ui, "RTPS MULTICAST 239.255.0.1:7400", theme::OUTLINE);
        let force = ui.button("Forçar probe (todos os nós)");
        if force.clicked() {
            let urls: Vec<String> = discovery.nodes.iter().map(|n| n.url.clone()).collect();
            for url in &urls {
                machines.probe_url(url);
            }
            crate::studio_log::info(format!(
                "máquinas: probe forçado de {} nó(s) pelo usuário",
                urls.len()
            ));
        }
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
    });
    ui.add_space(theme::SPACE_SM);

    if discovery.scanning && discovery.nodes.is_empty() {
        kit::empty_state(ui, "escutando o domínio…");
    }
    if !discovery.error.is_empty() {
        kit::error_banner(ui, &discovery.error);
    }

    let now_ns = crate::machines::now_unix_ns();

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
        ui.strong("Instalações Descobertas no Barramento");
        ui.add_space(theme::SPACE_XS);
        kit::legend(
            ui,
            &[
                ("autenticado (probe OK)", theme::OK),
                ("falha de auth (HTTP 401)", theme::AUTH),
                ("offline / porta recusada", theme::ERROR),
            ],
        );
        ui.add_space(theme::SPACE_SM);
        egui::Grid::new("discovery_grid")
            .striped(true)
            .show(ui, |ui| {
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
                    let alive = node.is_alive(now_ns);
                    let age = node.age_secs(now_ns);
                    let selected = discovery.selected == Some(index);

                    // Coluna 1: estado + node_id (+ badge ALVO PRIMÁRIO).
                    let (probe_state, probe_detail) = match &node.probe {
                        Some(probe) => (probe.state, probe.detail.clone()),
                        None => (ProbeState::Unknown, String::from("sondando…")),
                    };
                    let (state_color, state_dot) = match (&node.probe, alive) {
                        (Some(probe), _) => dot_style(probe.state),
                        (None, true) => (theme::WARN, "◐"),
                        (None, false) => (theme::STALE, "◌"),
                    };
                    ui.horizontal(|ui| {
                        ui.label(egui::RichText::new(state_dot).color(state_color).size(14.0));
                        kit::mono_cell(ui, &node.node_id);
                        if selected {
                            kit::badge(ui, "ALVO PRIMÁRIO", theme::PRIMARY_FIXED_DIM);
                        }
                    });
                    let _ = probe_state;

                    // Coluna 2: URL.
                    ui.hyperlink_to(&node.url, &node.url);

                    // Coluna 3: probe (protocolo ou motivo).
                    ui.label(
                        egui::RichText::new(match &node.probe {
                            Some(probe) => format!("● {}", probe.detail),
                            None => String::from("aguardando probe…"),
                        })
                        .monospace()
                        .small()
                        .color(state_color),
                    );

                    // Coluna 4: autenticação (401 é CREDENCIAL, não rede).
                    let has_token = machines.token_for(&node.url).is_some();
                    let (auth_text, auth_color) = match &node.probe {
                        Some(probe) if probe.state == ProbeState::AuthPending => {
                            (probe_detail.clone(), theme::AUTH)
                        }
                        Some(probe) if probe.state == ProbeState::Online => {
                            if node.token_required {
                                if has_token {
                                    ("Bearer em memória · aceito".to_owned(), theme::OK)
                                } else {
                                    ("sem token exigido pelo nó".to_owned(), theme::OK)
                                }
                            } else {
                                ("token não exigido".to_owned(), theme::OK)
                            }
                        }
                        Some(_) => (String::from("— sem resposta"), theme::ERROR),
                        None => (String::from("—"), theme::STALE),
                    };
                    ui.label(
                        egui::RichText::new(auth_text)
                            .monospace()
                            .small()
                            .color(auth_color),
                    );

                    // Coluna 5: heartbeat + ações.
                    let (hb, hb_color) = heartbeat_label(age);
                    ui.vertical(|ui| {
                        ui.label(
                            egui::RichText::new(format!("[{hb} {age}s]"))
                                .monospace()
                                .small()
                                .color(hb_color),
                        );
                        ui.horizontal(|ui| {
                            if selected {
                                ui.add_enabled(
                                    false,
                                    egui::Button::new(egui::RichText::new("Alvo Atual").small()),
                                );
                            } else if ui.button("Definir Alvo").clicked() {
                                discovery.selected = Some(index);
                                crate::studio_log::info(format!(
                                    "máquinas: alvo comutado para {}",
                                    node.url
                                ));
                            }
                            let needs_token = matches!(
                                &node.probe,
                                Some(p) if p.state == ProbeState::AuthPending
                            );
                            if needs_token && ui.button("Inserir token…").clicked() {
                                machines.modal_url = Some(node.url.clone());
                                machines.modal_token = String::new();
                            }
                            if ui.button("Re-testar").clicked() {
                                machines.probe_url(&node.url);
                            }
                        });
                    });
                    ui.end_row();
                }
            });
    }
    ui.add_space(theme::SPACE_LG);

    // ── Grid de 3: gauges de lease · classificador · resumo do domínio ──
    if !discovery.nodes.is_empty() {
        ui.columns(3, |cols| {
            // Janela de lease por nó (idade real / lease 10 s).
            cols[0].vertical(|ui| {
                ui.strong("Janela de Lease (10 s)");
                ui.add_space(theme::SPACE_XS);
                for node in &discovery.nodes {
                    let age = node.age_secs(now_ns) as f32;
                    let remaining = (10.0_f32 - age).max(0.0);
                    let (hb, color) = heartbeat_label(node.age_secs(now_ns));
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(&node.node_id)
                                .monospace()
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.add(
                            egui::ProgressBar::new((remaining / 10.0).clamp(0.0, 1.0))
                                .text(format!("{hb} · {remaining:.1}s"))
                                .desired_width(120.0)
                                .fill(color),
                        );
                    });
                }
            });
            // Classificador de falha semântica (401 ≠ offline).
            cols[1].vertical(|ui| {
                ui.strong("Classificador de Falha Semântica");
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
                ui.label(
                    egui::RichText::new(format!(
                        "● {auth_failures} falha(s) de AUTENTICAÇÃO (HTTP 401)"
                    ))
                    .monospace()
                    .small()
                    .color(theme::AUTH),
                );
                ui.label(
                    egui::RichText::new(format!("● {offline} nó(s) OFFLINE (rede/porta)"))
                        .monospace()
                        .small()
                        .color(theme::ERROR),
                );
                ui.weak(
                    "401 é estado de CREDENCIAL (token ausente/recusado); offline é \
                     rede — o barramento DDS continua operando nos dois casos.",
                );
            });
            // Resumo do domínio (não duplica o mesh da tela 3.11).
            cols[2].vertical(|ui| {
                ui.strong("Topologia do Domínio");
                ui.add_space(theme::SPACE_XS);
                let alive = discovery
                    .nodes
                    .iter()
                    .filter(|n| n.is_alive(now_ns))
                    .count();
                ui.label(
                    egui::RichText::new(format!(
                        "{}/{} instalações com heartbeat vivo",
                        alive,
                        discovery.nodes.len()
                    ))
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
                ui.label(
                    egui::RichText::new(format!(
                        "+ {} agente(s) · {} servidor(es) de inferência",
                        discovery.agents.len(),
                        discovery.servers.len()
                    ))
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
                );
                ui.weak("o mapa completo vive na tela 3.11 Topologia DDS.");
            });
        });
        ui.add_space(theme::SPACE_LG);
    }

    // ── Catálogo compartilhado persistido ──
    ui.collapsing("Catálogo Compartilhado (Persistido)", |ui| {
        ui.horizontal(|ui| {
            ui.label("autoridade:");
            ui.add_enabled_ui(!machines.busy, |ui| {
                ui.text_edit_singleline(&mut machines.url);
                if ui.button("Ler snapshot").clicked() {
                    machines.refresh();
                }
            });
            ui.label("token da autoridade:");
            ui.add(egui::TextEdit::singleline(&mut machines.authority_token).password(true));
        });
        if !machines.notice.is_empty() {
            ui.label(egui::RichText::new(&machines.notice).small().weak());
        }
        ui.separator();

        // Formulário de registro manual (token = password; nunca ao catálogo).
        ui.label("Registro manual (token fica só na memória local — nunca vai ao catálogo):");
        egui::Grid::new("machines_form_grid").show(ui, |ui| {
            ui.label("machine_id:");
            ui.text_edit_singleline(&mut machines.form_machine_id);
            ui.label("host:");
            ui.text_edit_singleline(&mut machines.form_host);
            ui.end_row();
            ui.label("user:");
            ui.text_edit_singleline(&mut machines.form_user);
            ui.label("node_url:");
            ui.text_edit_singleline(&mut machines.form_node_url);
            ui.end_row();
            ui.label("token:");
            ui.add(egui::TextEdit::singleline(&mut machines.form_token).password(true));
            ui.label("services_hint (csv):");
            ui.text_edit_singleline(&mut machines.form_services_hint);
            ui.end_row();
            ui.label("base:");
            ui.text_edit_singleline(&mut machines.form_base);
            ui.label("(vazia = criação; 409 preenche com a vigente)");
            ui.end_row();
        });
        ui.horizontal(|ui| {
            ui.add_enabled_ui(!machines.busy, |ui| {
                if ui.button("Publicar no catálogo").clicked() {
                    let _ = machines.publish_machine();
                }
                if ui.button("Guardar token em memória").clicked() {
                    machines.save_token_from_form();
                }
            });
        });
        if !machines.missing_token_urls().is_empty() {
            kit::warn_banner(
                ui,
                "token ausente: use “Inserir token…” na tabela acima (ou o \
                 formulário) para guardar o token SÓ na memória",
            );
        }
        ui.separator();

        let list = machines.machines();
        if list.is_empty() {
            kit::empty_state(
                ui,
                "Nenhuma máquina no snapshot. Clique Ler snapshot ou registre acima.",
            );
        } else {
            egui::Grid::new("machines_grid")
                .striped(true)
                .show(ui, |ui| {
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
                        ui.vertical(|ui| {
                            kit::mono_cell(ui, &machine.machine_id);
                            ui.label(
                                egui::RichText::new(format!("host: {}", machine.host))
                                    .small()
                                    .weak(),
                            );
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
                                ProbeState::Unknown => kit::badge(ui, "SEM_RESPOSTA", theme::STALE),
                            },
                        }
                        ui.add_enabled_ui(!machines.busy, |ui| {
                            if ui.button("Probe").clicked() {
                                machines.probe(machine);
                            }
                        });
                        ui.end_row();
                    }
                });
            ui.label(
                egui::RichText::new("~/.dds_studio/nodes_catalog.json (autoridade do nó alvo)")
                    .small()
                    .weak(),
            );
        }
    });

    // ── Modal: Definir como Alvo & Configurar Token ──
    let modal_url = machines.modal_url.clone();
    if let Some(url) = modal_url {
        let mut close = false;
        egui::Window::new("Definir como Alvo & Configurar Token")
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
