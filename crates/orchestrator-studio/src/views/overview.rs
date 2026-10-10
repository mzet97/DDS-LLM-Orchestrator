//! Painel de visão geral (3.1): agregado honesto do sistema em um relance.

use crate::agents::AgentsState;
use crate::catalog_remote::SharedCatalog;
use crate::discovery::DiscoveryState;
use crate::kit;
use crate::machines::{now_unix_ns, ProbeState};
use crate::models::{ManifestStatus, ModelsState};
use crate::overview::{summarize, OverviewInput, TileHealth};
use crate::services::ServicesPanel;
use crate::state::AppState;
use crate::theme;
use eframe::egui;

/// Dot do cartão pela saúde agregada (tokens do design system).
fn health_dot(health: TileHealth, stale: bool) -> egui::Color32 {
    if stale || health == TileHealth::Stale {
        return theme::STALE;
    }
    match health {
        TileHealth::Ok => theme::OK,
        TileHealth::Warn => theme::WARN,
        TileHealth::Stale => theme::STALE,
    }
}

/// Card de telemetria do design (título + status, subtítulo, linhas, rodapé).
struct TeleCard {
    dot: egui::Color32,
    title: String,
    status: String,
    status_color: egui::Color32,
    subtitle: String,
    rows: Vec<(String, String, egui::Color32)>,
    footer: (String, String),
}

fn tele_card(ui: &mut egui::Ui, card: &TeleCard) {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                let (dot_rect, _) =
                    ui.allocate_exact_size(egui::vec2(10.0, 18.0), egui::Sense::hover());
                ui.painter().circle_filled(dot_rect.center(), 4.0, card.dot);
                ui.label(
                    egui::RichText::new(&card.title)
                        .size(13.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(&card.status)
                            .size(11.5)
                            .color(card.status_color),
                    );
                });
            });
            ui.label(
                egui::RichText::new(&card.subtitle)
                    .size(11.0)
                    .color(theme::OUTLINE),
            );
            ui.add_space(theme::SPACE_XS);
            egui::Frame::NONE
                .fill(theme::SURFACE_LOW)
                .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
                .inner_margin(egui::Margin::symmetric(8, 6))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    for (label, value, color) in &card.rows {
                        ui.horizontal(|ui| {
                            ui.label(egui::RichText::new(label).size(11.0).color(theme::OUTLINE));
                            ui.with_layout(
                                egui::Layout::right_to_left(egui::Align::Center),
                                |ui| {
                                    ui.label(
                                        egui::RichText::new(value)
                                            .monospace()
                                            .size(11.0)
                                            .color(*color),
                                    );
                                },
                            );
                        });
                    }
                });
            ui.add_space(theme::SPACE_XS);
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&card.footer.0)
                        .size(11.0)
                        .color(theme::OUTLINE),
                );
                ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                    ui.label(
                        egui::RichText::new(&card.footer.1)
                            .monospace()
                            .size(11.0)
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                });
            });
        });
}

/// Card de descoberta do hero (kicker + número + nome + linhas mono).
struct HeroCard {
    kicker: String,
    count: String,
    name: String,
    lines: Vec<(String, String)>,
}

fn hero_card(ui: &mut egui::Ui, card: &HeroCard) {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.label(
                egui::RichText::new(&card.kicker)
                    .monospace()
                    .size(10.5)
                    .color(theme::OUTLINE),
            );
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new(&card.count)
                        .size(30.0)
                        .strong()
                        .color(theme::PRIMARY_FIXED_DIM),
                );
                ui.label(
                    egui::RichText::new(&card.name)
                        .size(15.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
            });
            for (left, right) in &card.lines {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(left)
                            .monospace()
                            .size(10.5)
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        ui.label(
                            egui::RichText::new(right)
                                .monospace()
                                .size(10.5)
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                    });
                });
            }
        });
}

/// Cartão 1 — Nó studio-node (daemon local + protocolo + operações + EMA).
fn node_card(state: &AppState, discovery: &DiscoveryState, dot: egui::Color32) -> TeleCard {
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
fn services_card(services: &ServicesPanel, dot: egui::Color32) -> TeleCard {
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
fn agents_card(agents: &AgentsState, discovery: &DiscoveryState, dot: egui::Color32) -> TeleCard {
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

/// Cartão 4 — Modelos GGUF (inventário × manifesto SHA-256).
fn models_card(models: &ModelsState, dot: egui::Color32) -> TeleCard {
    let cyan = theme::PRIMARY_FIXED_DIM;
    let ok = models
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Ok)
        .count();
    let deviated = models
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Desviado)
        .count();
    let pending_name = models
        .list
        .iter()
        .find(|item| item.manifest_status == ManifestStatus::Pendente)
        .map(|item| item.file_name.chars().take(20).collect());
    let volume_gb = models.list.iter().map(|item| item.size_bytes).sum::<u64>() as f64 / 1e9;
    TeleCard {
        dot,
        title: String::from("Modelos GGUF"),
        status: if models.list.is_empty() {
            String::from("Vazio")
        } else {
            format!("{} no Inventário", models.list.len())
        },
        status_color: if models.list.is_empty() {
            theme::STALE
        } else {
            theme::ON_SURFACE_VARIANT
        },
        subtitle: String::from("Repositório de Pesos & Manifestos SHA256"),
        rows: vec![
            (
                String::from("Conformidade SHA:"),
                if models.list.is_empty() {
                    String::from("—")
                } else {
                    format!("{ok} OK · {deviated} DESVIADO")
                },
                if deviated > 0 {
                    theme::WARN
                } else if models.list.is_empty() {
                    theme::STALE
                } else {
                    cyan
                },
            ),
            (
                String::from("Checksum Pendente:"),
                pending_name.unwrap_or_else(|| String::from("—")),
                cyan,
            ),
            (
                String::from("Manifesto:"),
                if models.manifest_path.is_empty() {
                    String::from("nenhum")
                } else {
                    models
                        .manifest_path
                        .rsplit('/')
                        .next()
                        .unwrap_or(&models.manifest_path)
                        .chars()
                        .take(20)
                        .collect()
                },
                cyan,
            ),
        ],
        footer: (
            String::from("Volume Total:"),
            format!("{volume_gb:.1} GB mapeados"),
        ),
    }
}

/// Cartão 5 — Inferência via presença DDS (ServerStatus) + prova local.
fn inference_card(discovery: &DiscoveryState, proof: &str, dot: egui::Color32) -> TeleCard {
    let cyan = theme::PRIMARY_FIXED_DIM;
    match discovery.servers.first() {
        Some(server) => TeleCard {
            dot,
            title: String::from("Inferência (Llama-Server)"),
            status: String::from("Presença DDS Ok"),
            status_color: theme::OK,
            subtitle: String::from("Engine de Inferência Local"),
            rows: vec![
                (
                    String::from("Modelo Carregado:"),
                    server.model_loaded.chars().take(20).collect(),
                    cyan,
                ),
                (
                    String::from("Slots de Contexto:"),
                    format!(
                        "{}/{} ({} livres)",
                        server.slots_processing,
                        server.slots_idle + server.slots_processing,
                        server.slots_idle
                    ),
                    cyan,
                ),
                (
                    String::from("API Compatível:"),
                    String::from("OpenAI /v1/chat/completions"),
                    cyan,
                ),
            ],
            footer: (
                String::from("Prova local:"),
                if proof.is_empty() {
                    String::from("—")
                } else {
                    proof.chars().take(28).collect()
                },
            ),
        },
        None => TeleCard {
            dot,
            title: String::from("Inferência (Llama-Server)"),
            status: String::from("Sem presença"),
            status_color: theme::STALE,
            subtitle: String::from("Engine de Inferência Local"),
            rows: vec![
                (
                    String::from("Modelo Carregado:"),
                    String::from("—"),
                    theme::STALE,
                ),
                (
                    String::from("Slots de Contexto:"),
                    String::from("—"),
                    theme::STALE,
                ),
                (
                    String::from("API Compatível:"),
                    String::from("OpenAI /v1/chat/completions"),
                    cyan,
                ),
            ],
            footer: (
                String::from("Prova local:"),
                if proof.is_empty() {
                    String::from("—")
                } else {
                    proof.chars().take(28).collect()
                },
            ),
        },
    }
}

/// Cartão 6 — Catálogo compartilhado (snapshot + cursor + subdivisão real).
fn catalog_card(shared: &SharedCatalog, summary: &str, dot: egui::Color32) -> TeleCard {
    let cyan = theme::PRIMARY_FIXED_DIM;
    let items = shared
        .snapshot
        .as_ref()
        .map(|snapshot| snapshot.items.as_slice())
        .unwrap_or(&[]);
    // Subdivisão honesta: conta por primeiro segmento do id ("machine/…").
    let mut groups: std::collections::BTreeMap<&str, usize> = std::collections::BTreeMap::new();
    for item in items {
        let segment = item.id.0.split('/').next().unwrap_or("?");
        *groups.entry(segment).or_default() += 1;
    }
    let mut ranked: Vec<(&str, usize)> = groups.into_iter().collect();
    ranked.sort_by(|a, b| b.1.cmp(&a.1).then_with(|| a.0.cmp(b.0)));
    let subdivision = if ranked.is_empty() {
        String::from("—")
    } else {
        ranked
            .iter()
            .take(2)
            .map(|(segment, count)| format!("{segment} ({count})"))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    TeleCard {
        dot,
        title: String::from("Catálogo Compartilhado"),
        status: shared
            .snapshot
            .as_ref()
            .map(|snapshot| format!("Snapshot cursor {}", snapshot.cursor.0))
            .unwrap_or_else(|| String::from("Aguardando")),
        status_color: if shared.snapshot.is_some() {
            theme::ON_SURFACE_VARIANT
        } else {
            theme::STALE
        },
        subtitle: String::from("Registro Central Distribuído de Estado"),
        rows: vec![
            (
                String::from("Total Registros:"),
                format!("{} itens mapeados", items.len()),
                cyan,
            ),
            (String::from("Subdivisão:"), subdivision, cyan),
            (
                String::from("Autoridade:"),
                if shared.url.is_empty() {
                    String::from("—")
                } else {
                    kit::authority(&shared.url)
                },
                cyan,
            ),
        ],
        footer: (String::from("Snapshot:"), summary.to_owned()),
    }
}

#[allow(clippy::too_many_arguments)]
pub fn show(
    ui: &mut egui::Ui,
    state: &mut AppState,
    services: &mut ServicesPanel,
    agents: &mut AgentsState,
    models: &ModelsState,
    shared: &mut SharedCatalog,
    proof: &str,
    discovery: &DiscoveryState,
) {
    let target = discovery.selected_url();

    // ── Faixa de alvo ativo (design: pill fina de uma linha) ──
    let now = now_unix_ns();
    egui::Frame::NONE
        .fill(theme::tint(theme::PRIMARY_CONTAINER, 8))
        .stroke(egui::Stroke::new(
            1.0,
            theme::tint(theme::PRIMARY_CONTAINER, 45),
        ))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::symmetric(12, 7))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("●")
                        .small()
                        .color(theme::PRIMARY_CONTAINER),
                );
                match (&target, state.node()) {
                    (Some(url), Some(node)) => {
                        ui.label(
                            egui::RichText::new(format!("ALVO ATIVO: {url}"))
                                .monospace()
                                .size(11.0)
                                .color(theme::PRIMARY_FIXED_DIM),
                        );
                        ui.label(
                            egui::RichText::new(format!(
                                "● Conectado ao studio-node v{}.{} · {} ops registradas",
                                node.version.major,
                                node.version.minor,
                                node.operations.len()
                            ))
                            .monospace()
                            .size(11.0)
                            .color(theme::OK),
                        );
                        let ttl = discovery
                            .nodes
                            .iter()
                            .find(|n| n.url == *url)
                            .map(|n| 10u64.saturating_sub(n.age_secs(now)));
                        ui.label(
                            egui::RichText::new(format!(
                                "QoS Lease: 10.0s [TTL: {}]",
                                ttl.map(|t| format!("{t}s"))
                                    .unwrap_or_else(|| String::from("—"))
                            ))
                            .monospace()
                            .size(11.0)
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                        ui.label(
                            egui::RichText::new("Auto-Carga Contínua (5s)")
                                .monospace()
                                .size(11.0)
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                    }
                    _ => {
                        ui.label(
                            egui::RichText::new(format!(
                                "○ ALVO ATIVO: — nenhum alvo — · descoberta contínua \
                                 no domínio {}",
                                discovery.domain
                            ))
                            .monospace()
                            .size(11.0)
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                    }
                }
            });
        });
    ui.add_space(theme::SPACE_SM);

    // ── Hero de descoberta (design: título + Ativo + pill QoS) ──
    let mut heartbeat_ns: Vec<u64> = discovery
        .nodes
        .iter()
        .map(|n| n.last_seen_unix_ns)
        .chain(discovery.agents.iter().map(|a| a.last_update_ns))
        .collect();
    heartbeat_ns.sort_unstable();
    let freshest_age = heartbeat_ns
        .last()
        .map(|newest| now.saturating_sub(*newest) / 1_000_000_000);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("●")
                .size(16.0)
                .color(theme::PRIMARY_CONTAINER),
        );
        ui.label(
            egui::RichText::new(format!(
                "Domínio DDS {} · Descoberta em Tempo Real",
                discovery.domain
            ))
            .size(19.0)
            .strong()
            .color(theme::ON_SURFACE),
        );
        if let Some(age) = freshest_age {
            kit::badge(ui, &format!("● Ativo há {age}s"), theme::OK);
        } else {
            kit::badge(ui, "○ Sem dados", theme::STALE);
        }
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            kit::badge(ui, "QoS RELIABLE + TRANSIENT_LOCAL", theme::OUTLINE);
        });
    });
    ui.add_space(theme::SPACE_SM);

    // ── 3 cards de descoberta (design: barramento + registry + endpoint) ──
    let node_lines: Vec<(String, String)> = discovery
        .nodes
        .iter()
        .take(3)
        .map(|n| {
            (
                n.node_id.chars().take(21).collect(),
                n.probe
                    .as_ref()
                    .map(|p| p.detail.chars().take(18).collect())
                    .unwrap_or_else(|| String::from("sondando…")),
            )
        })
        .collect();
    let agent_lines: Vec<(String, String)> = discovery
        .agents
        .iter()
        .take(3)
        .map(|a| {
            (
                a.agent_id.chars().take(18).collect(),
                format!("slots {}/{}", a.slots_busy, a.slots_total),
            )
        })
        .collect();
    let server_lines: Vec<(String, String)> = discovery
        .servers
        .first()
        .map(|s| {
            vec![
                (
                    s.server_id.chars().take(18).collect(),
                    if s.ready {
                        String::from("pronto")
                    } else {
                        String::from("não pronto")
                    },
                ),
                (
                    s.model_loaded.chars().take(18).collect(),
                    format!(
                        "slots {}/{}",
                        s.slots_processing,
                        s.slots_idle + s.slots_processing
                    ),
                ),
                (String::from("Publica"), String::from("ServerStatus")),
            ]
        })
        .unwrap_or_else(|| vec![(String::from("—"), String::from("sem presença"))]);
    ui.columns(3, |cols| {
        hero_card(
            &mut cols[0],
            &HeroCard {
                kicker: String::from("TOPOLOGIA DE BARRAMENTO"),
                count: discovery.nodes.len().to_string(),
                name: String::from("Nós Studio"),
                lines: if node_lines.is_empty() {
                    vec![(String::from("—"), String::from("sem nós"))]
                } else {
                    node_lines
                },
            },
        );
        hero_card(
            &mut cols[1],
            &HeroCard {
                kicker: String::from("AGENTREGISTRY"),
                count: discovery.agents.len().to_string(),
                name: String::from("Agentes IA"),
                lines: if agent_lines.is_empty() {
                    vec![(String::from("—"), String::from("sem agentes"))]
                } else {
                    agent_lines
                },
            },
        );
        hero_card(
            &mut cols[2],
            &HeroCard {
                kicker: String::from("SERVERSTATUS ENDPOINT"),
                count: discovery.servers.len().to_string(),
                name: String::from("Servidor Inferência"),
                lines: server_lines,
            },
        );
    });
    ui.add_space(theme::SPACE_MD);

    // ── Despacho operacional imediato (design: rótulo + 2 fantasmas + 1 ciano) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Despacho Operacional Imediato:")
                .size(12.5)
                .color(theme::ON_SURFACE_VARIANT),
        );
        let reload = ui.button("Re-carregar alvo (Nó · Serviços · Catálogo)");
        let read_plan = ui.button("Ler plano de serviços");
        let refresh_orch = kit::primary_button(ui, "↑ Atualizar Orquestrador");
        if reload.clicked() || read_plan.clicked() {
            if let Some(target) = discovery.selected_url() {
                let token =
                    crate::discovery::token_for_url(&target, std::env::var("HOME").ok().as_deref());
                if reload.clicked() {
                    state.refresh_from_node_with_token(&target, token.as_deref());
                }
                services.refresh();
                if reload.clicked() {
                    shared.refresh();
                }
            }
            crate::studio_log::info(String::from("visão geral: releitura do alvo solicitada"));
        }
        if refresh_orch.clicked() {
            agents.refresh();
            crate::studio_log::info(String::from("visão geral: orquestrador atualizado"));
        }
    });
    ui.add_space(theme::SPACE_MD);

    let hashed = models
        .list
        .iter()
        .filter(|item| !item.sha256_hex.is_empty())
        .count();
    let (catalog_items, catalog_cursor, catalog_loaded) = match &shared.snapshot {
        Some(snapshot) => (snapshot.items.len(), snapshot.cursor.0, true),
        None => (0, 0, false),
    };
    let tiles = summarize(&OverviewInput {
        node: state.node(),
        node_error: "",
        services: &services.list,
        services_error: &services.error,
        agents: &agents.list,
        agents_error: &agents.error,
        models_total: models.list.len(),
        models_hashed: hashed,
        inference_proof: proof,
        discovery_nodes: discovery.nodes.len(),
        discovery_agents: discovery.agents.len(),
        discovery_servers: discovery.servers.len(),
        discovery_target: target.as_deref(),
        catalog_items,
        catalog_cursor,
        catalog_loaded,
        catalog_error: &shared.notice,
    });

    // ── Fontes de dados & telemetria (design: título + badge + dots) ──
    let ok_count = tiles
        .iter()
        .filter(|tile| !tile.stale && tile.health == TileHealth::Ok)
        .count();
    let warn_count = tiles
        .iter()
        .filter(|tile| !tile.stale && tile.health == TileHealth::Warn)
        .count();
    let stale_count = tiles.len() - ok_count - warn_count;
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Fontes de Dados do Sistema & Telemetria")
                .size(15.0)
                .strong()
                .color(theme::ON_SURFACE),
        );
        kit::badge(
            ui,
            &format!("{} Sensores Ativos", ok_count + warn_count),
            theme::OUTLINE,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{stale_count} Stale"))
                    .size(11.5)
                    .color(theme::ON_SURFACE_VARIANT),
            );
            ui.label(egui::RichText::new("●").size(11.5).color(theme::STALE));
            ui.label(
                egui::RichText::new(format!("{warn_count} Warn"))
                    .size(11.5)
                    .color(theme::ON_SURFACE_VARIANT),
            );
            ui.label(egui::RichText::new("●").size(11.5).color(theme::WARN));
            ui.label(
                egui::RichText::new(format!("{ok_count} Ok"))
                    .size(11.5)
                    .color(theme::ON_SURFACE_VARIANT),
            );
            ui.label(egui::RichText::new("●").size(11.5).color(theme::OK));
        });
    });
    ui.add_space(theme::SPACE_SM);

    // ── 6 cartões ricos (design: título + status, linhas, rodapé) ──
    let dots: Vec<egui::Color32> = tiles
        .iter()
        .map(|tile| health_dot(tile.health, tile.stale))
        .collect();
    let cards = [
        node_card(state, discovery, dots[0]),
        services_card(services, dots[1]),
        agents_card(agents, discovery, dots[2]),
        models_card(models, dots[3]),
        inference_card(discovery, proof, dots[4]),
        catalog_card(shared, &tiles[5].summary, dots[5]),
    ];
    ui.columns(3, |cols| {
        tele_card(&mut cols[0], &cards[0]);
        tele_card(&mut cols[1], &cards[1]);
        tele_card(&mut cols[2], &cards[2]);
    });
    ui.add_space(theme::SPACE_SM);
    ui.columns(3, |cols| {
        tele_card(&mut cols[0], &cards[3]);
        tele_card(&mut cols[1], &cards[4]);
        tele_card(&mut cols[2], &cards[5]);
    });
    ui.add_space(theme::SPACE_LG);

    // ── Participantes do barramento (design: 5 colunas + domínio) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("Topologia de Barramento DDS · Participantes & Partições")
                .size(14.0)
                .strong()
                .color(theme::ON_SURFACE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "DOMÍNIO {} · {} PARTICIPANTES",
                    discovery.domain,
                    discovery.nodes.len()
                ))
                .monospace()
                .size(11.0)
                .color(theme::OUTLINE),
            );
        });
    });
    ui.add_space(theme::SPACE_SM);
    if discovery.nodes.is_empty() {
        kit::empty_state(
            ui,
            &format!(
                "Nenhum participante visto no domínio {} — nós com canvas DDS \
                 publicam presença sozinhos.",
                discovery.domain
            ),
        );
    } else {
        let now = now_unix_ns();
        kit::table("overview_nodes_grid").show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "NÓ / PARTICIPANTE",
                    "ENDEREÇO / PORTA",
                    "PROBE",
                    "ÚLTIMO HEARTBEAT",
                    "ESTADO QoS",
                ],
            );
            for node in &discovery.nodes {
                kit::mono_cell(ui, &node.node_id);
                kit::mono_cell(ui, &node.url);
                let (color, detail) = match &node.probe {
                    Some(probe) => match probe.state {
                        ProbeState::Online => (theme::OK, probe.detail.clone()),
                        ProbeState::AuthPending => (theme::AUTH, probe.detail.clone()),
                        ProbeState::Offline | ProbeState::Unknown => {
                            (theme::ERROR, probe.detail.clone())
                        }
                    },
                    None => (theme::STALE, String::from("sondando…")),
                };
                ui.label(
                    egui::RichText::new(format!("● {detail}"))
                        .monospace()
                        .small()
                        .color(color),
                );
                kit::num_cell(ui, &format!("há {}s", node.age_secs(now)));
                kit::mono_cell(ui, "RELIABLE / TRANSIENT");
                ui.end_row();
            }
        });
    }
}
