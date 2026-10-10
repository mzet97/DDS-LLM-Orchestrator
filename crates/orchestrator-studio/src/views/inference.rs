//! Painel 3.3 Inferência & Chat de Engenharia: fiel ao mockup Stitch —
//! cabeçalho com planos DDS/HTTP, descoberta ServerStatus (linha por
//! servidor), endpoint HTTP + model probe, parâmetros de geração, transcript
//! com stats reais e composer com Ctrl+Enter.
//!
//! Só dados reais: sem GUID de servidor (o fio não tem), sem build/CUDA do
//! alvo (o `/v1/models` não anuncia), sem VRAM/slots inventados e sem
//! denominador de contexto (o servidor não informa `n_ctx`). O QoS do
//! `ServerStatus` segue o CÓDIGO (`qos.rs`: BestEffort + Volatile +
//! KeepLast(1)) — o mockup diz "TransientLocal · Reliable", o que contradiz
//! o contrato e a si mesmo (durabilidades alternativas).

use crate::discovery::DiscoveryState;
use crate::inference::{InferenceState, Role};
use crate::kit;
use crate::theme;
use eframe::egui;

/// Moldura dos cards da 3.3 (mesmo idioma da 3.2/3.10).
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

/// Relógio de recebimento (ms unix → HH:MM:SS UTC, conta manual).
fn hhmmss_utc(ts_unix_ms: u64) -> String {
    let secs = ts_unix_ms / 1000;
    format!(
        "{:02}:{:02}:{:02} UTC",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60
    )
}

/// Legenda honesta da temperatura (faixas documentadas da GUI).
fn temp_caption(temperature: f32) -> &'static str {
    if temperature < 0.3 {
        "Preciso / baixo jitter"
    } else if temperature < 1.0 {
        "Equilibrado"
    } else {
        "Criativo / dispersivo"
    }
}

/// Modo do mockup derivado da temperatura real (faixas da GUI).
fn temp_mode(temperature: f32) -> &'static str {
    if temperature <= 0.0 {
        "DETERMINÍSTICO"
    } else if temperature < 0.7 {
        "FOCADO"
    } else {
        "EXPLORATÓRIO"
    }
}

/// Slider com trilho visível no card: o tema global pinta `inactive` com a
/// mesma cor do card (certo para botões/campos) — sem isto o trilho some e
/// só o pomo aparece.
fn card_slider(ui: &mut egui::Ui, value: &mut f32, range: std::ops::RangeInclusive<f32>) {
    ui.scope(|ui| {
        ui.style_mut().visuals.widgets.inactive.bg_fill = theme::SURFACE_HIGHEST;
        ui.style_mut().spacing.slider_width = 150.0;
        ui.add(egui::Slider::new(value, range).show_value(false));
    });
}

/// Tokens conhecidos da sessão (soma do `usage` real) + nº de turnos.
fn session_tokens(infer: &InferenceState) -> (u64, usize) {
    let mut sum = 0u64;
    for stats in &infer.stats {
        sum += stats.prompt_tokens.unwrap_or(0) + stats.completion_tokens.unwrap_or(0);
    }
    (sum, infer.stats.len())
}

pub fn show(ui: &mut egui::Ui, infer: &mut InferenceState, discovery: &DiscoveryState) {
    infer.poll();
    if infer.busy {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    } else {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_secs(1));
    }

    let http_live = infer.last_verify_ms.is_some() && !infer.models.is_empty();

    // ── Cabeçalho (mockup; shell global novo já vem do main) ──
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.horizontal(|ui| {
                ui.label(
                    egui::RichText::new("3.3 Inferência & Chat de Engenharia")
                        .size(22.0)
                        .strong()
                        .color(theme::ON_SURFACE),
                );
                kit::badge(ui, "DDS_TOPIC: Studio.ServerStatus", theme::OUTLINE);
            });
            ui.label(
                egui::RichText::new(
                    "Dois planos: presença DDS (ServerStatus) × chat HTTP (endpoint explícito)",
                )
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
        });
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.vertical(|ui| {
                ui.label(
                    egui::RichText::new("REQ/T-820-19: T_MAX 120s")
                        .monospace()
                        .small()
                        .color(theme::OUTLINE),
                );
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(if discovery.servers.is_empty() {
                            "○ PLANO DDS: NENHUM SERVIDOR VISTO"
                        } else {
                            "● PLANO DDS: PARTICIPANTE VISTO"
                        })
                        .monospace()
                        .small()
                        .strong()
                        .color(if discovery.servers.is_empty() {
                            theme::STALE
                        } else {
                            theme::PRIMARY_FIXED_DIM
                        }),
                    );
                    ui.label(
                        egui::RichText::new(if http_live {
                            "● PLANO HTTP: ROTAS V1 ATIVAS"
                        } else {
                            "○ PLANO HTTP: ROTAS V1 NÃO VERIFICADAS"
                        })
                        .monospace()
                        .small()
                        .strong()
                        .color(if http_live {
                            theme::OK
                        } else {
                            theme::WARN
                        }),
                    );
                });
            });
        });
    });
    ui.add_space(theme::SPACE_SM);

    // ── Descoberta de inferência no domínio (mockup) ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(
                "● DESCOBERTA DE INFERÊNCIA NO DOMÍNIO (SERVERSTATUS DDS VS. HTTP GATEWAY)",
            )
            .monospace()
            .size(11.5)
            .strong()
            .color(theme::PRIMARY_FIXED_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new("QoS: BestEffort · Volatile · KeepLast(1) (heartbeat)")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    card_frame().show(ui, |ui| {
        ui.set_min_width(ui.available_width());
        if discovery.servers.is_empty() {
            kit::empty_state(
                ui,
                &format!(
                    "Nenhum servidor anunciando ServerStatus no domínio {} — o \
                     llama-server precisa rodar com o DDS ligado para aparecer aqui.",
                    discovery.domain
                ),
            );
        } else {
            kit::table("infer_servers_grid").show(ui, |ui| {
                kit::grid_header(
                    ui,
                    &[
                        "SERVIDOR DESCOBERTO",
                        "MODELO CARREGADO NA VRAM",
                        "KV SLOTS DISPONÍVEIS",
                        "STATUS NO BARRAMENTO DDS",
                    ],
                );
                for server in &discovery.servers {
                    kit::mono_cell(ui, &server.server_id);
                    ui.label(
                        egui::RichText::new(if server.model_loaded.is_empty() {
                            "(não anunciado)"
                        } else {
                            server.model_loaded.as_str()
                        })
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                    let total = server.slots_idle + server.slots_processing;
                    let full = total > 0 && server.slots_idle == 0;
                    ui.label(
                        egui::RichText::new(format!(
                            "{}/{} ATIVOS ({} LIVRES){}",
                            server.slots_processing,
                            total,
                            server.slots_idle,
                            if full {
                                " · LOTADO"
                            } else if server.slots_processing > 0 {
                                " (fila local ativa)"
                            } else {
                                ""
                            }
                        ))
                        .monospace()
                        .small()
                        .color(if full {
                            theme::ERROR
                        } else {
                            theme::ON_SURFACE_VARIANT
                        }),
                    );
                    ui.label(
                        egui::RichText::new(if server.ready {
                            "● Pronto (lease 10s do tópico)"
                        } else {
                            "○ Não pronto"
                        })
                        .monospace()
                        .small()
                        .strong()
                        .color(if server.ready {
                            theme::PRIMARY_FIXED_DIM
                        } else {
                            theme::WARN
                        }),
                    );
                    ui.end_row();
                }
            });
        }
        ui.add_space(theme::SPACE_XS);
        ui.label(
            egui::RichText::new(
                "Postura de acoplamento §3.3: presença no barramento DDS confirmada \
                 via tópico Studio.ServerStatus (heartbeats BestEffort), mas o \
                 endpoint HTTP exige conexão explícita configurada abaixo. O \
                 contrato ServerStatus não divulga intencionalmente porta/URL \
                 HTTP (isolamento de superfície de ataque): aponte o pipeline \
                 /v1/chat/completions para o endereço RPC do servidor.",
            )
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
    });
    ui.add_space(theme::SPACE_LG);

    // ── Endpoint HTTP + parâmetros de geração (mockup) ──
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(
                    ui,
                    "ENDPOINT DE CONTROLE HTTP & MODEL PROBE",
                    "TLS OPCIONAL · LOOPBACK/LOCAL LAN",
                );
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("URL:")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.add_enabled_ui(!infer.busy, |ui| {
                        ui.add(
                            egui::TextEdit::singleline(&mut infer.server_url)
                                .desired_width(ui.available_width() - 170.0)
                                .hint_text("http://192.168.1.61:8081"),
                        );
                        if ui.button("Verificar /v1/models").clicked() {
                            infer.refresh_models();
                        }
                    });
                });
                ui.add_space(theme::SPACE_XS);
                match (infer.last_verify_ms, infer.models.is_empty()) {
                    (Some(ms), false) => {
                        ui.label(
                            egui::RichText::new(format!(
                                "● HTTP 200 OK · {ms}ms · {} modelo(s): {}",
                                infer.models.len(),
                                infer.models.join(" · ")
                            ))
                            .monospace()
                            .small()
                            .color(theme::OK),
                        );
                    }
                    _ if infer.busy && infer.reply == "listando modelos…" => {
                        ui.label(
                            egui::RichText::new("○ Verificando /v1/models…")
                                .monospace()
                                .small()
                                .color(theme::WARN),
                        );
                    }
                    _ if infer.reply.starts_with("erro") => {
                        ui.label(
                            egui::RichText::new(format!("○ {}", infer.reply))
                                .monospace()
                                .small()
                                .color(theme::ERROR),
                        );
                    }
                    _ => {
                        ui.label(
                            egui::RichText::new(
                                "○ Sem verificação (informe a URL e clique Verificar)",
                            )
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                        );
                    }
                }
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("Modelo Ativo:")
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    if infer.models.is_empty() {
                        ui.label(
                            egui::RichText::new("— (verifique /v1/models)")
                                .small()
                                .weak(),
                        );
                    } else {
                        egui::ComboBox::from_id_salt("infer-model")
                            .selected_text(if infer.model.is_empty() {
                                "—".to_owned()
                            } else {
                                infer.model.clone()
                            })
                            .show_ui(ui, |ui| {
                                for model in &infer.models {
                                    ui.selectable_value(
                                        &mut infer.model,
                                        model.clone(),
                                        model.as_str(),
                                    );
                                }
                            });
                    }
                });
            });
        });
        cols[1].vertical(|ui| {
            card_frame().show(ui, |ui| {
                ui.set_min_width(ui.available_width());
                card_title(
                    ui,
                    "PARÂMETROS DE GERAÇÃO (QOS T-820)",
                    temp_mode(infer.temperature),
                );
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("TEMPERATURA  {:.2}", infer.temperature))
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    card_slider(ui, &mut infer.temperature, 0.0..=2.0);
                    ui.label(
                        egui::RichText::new(temp_caption(infer.temperature))
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("MAX TOKENS  {}", infer.max_tokens))
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.add(egui::DragValue::new(&mut infer.max_tokens).range(1..=8192));
                    egui::ComboBox::from_id_salt("infer-maxtok")
                        .selected_text(format!("{} tk", infer.max_tokens))
                        .show_ui(ui, |ui| {
                            for preset in [256u32, 512, 1024, 2048, 4096, 8192] {
                                ui.selectable_value(
                                    &mut infer.max_tokens,
                                    preset,
                                    format!("{preset} tk"),
                                );
                            }
                        });
                    ui.label(
                        egui::RichText::new("Limite de contexto")
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("TOP-P  {:.2}", infer.top_p))
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    card_slider(ui, &mut infer.top_p, 0.0..=1.0);
                    ui.label(
                        egui::RichText::new("Nucleus sampling")
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new("TIMEOUT (S)")
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    );
                    ui.label(
                        egui::RichText::new("120 seg (fixo)")
                            .monospace()
                            .small()
                            .strong()
                            .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.label(
                        egui::RichText::new("Espera segura (REQ/T-820-19)")
                            .small()
                            .color(theme::OUTLINE),
                    );
                });
            });
        });
    });
    ui.add_space(theme::SPACE_LG);

    // ── Transcript de engenharia & depuração (mockup) ──
    let (session_toks, turns) = session_tokens(infer);
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("TRANSCRIPT DE ENGENHARIA & DEPURAÇÃO")
                .monospace()
                .size(11.5)
                .strong()
                .color(theme::ON_SURFACE),
        );
        ui.label(
            egui::RichText::new(if turns == 0 {
                String::from("SESSÃO VAZIA")
            } else {
                format!("TOKENS NA SESSÃO (USAGE REAL): {session_toks} · {turns} turno(s)")
            })
            .monospace()
            .small()
            .color(theme::PRIMARY_FIXED_DIM),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Exportar JSON").clicked() {
                match infer.export_session(&std::env::temp_dir()) {
                    Ok(path) => {
                        infer.reply = format!("transcript exportado: {}", path.display());
                        crate::studio_log::info(format!(
                            "inferência: transcript exportado para {}",
                            path.display()
                        ));
                    }
                    Err(err) => {
                        infer.reply = format!("falha ao exportar: {err}");
                    }
                }
            }
            if ui.button("Limpar Histórico").clicked() {
                infer.clear_session();
            }
        });
    });
    ui.add_space(theme::SPACE_XS);
    egui::ScrollArea::vertical()
        .id_salt("infer-transcript")
        .max_height(380.0)
        .stick_to_bottom(true)
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            if infer.history.is_empty() {
                kit::empty_state(ui, "sessão vazia — envie o primeiro prompt abaixo.");
            }
            let mut assistant_index = 0usize;
            for message in &infer.history {
                match message.role {
                    Role::System => {
                        ui.label(
                            egui::RichText::new(format!("sistema: {}", message.content)).weak(),
                        );
                    }
                    Role::User => {
                        card_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                kit::badge(ui, "OP", theme::PRIMARY_FIXED_DIM);
                                ui.label(
                                    egui::RichText::new("você (operador local)")
                                        .small()
                                        .strong()
                                        .color(theme::ON_SURFACE),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        ui.label(
                                            egui::RichText::new(hhmmss_utc(message.ts_unix_ms))
                                                .monospace()
                                                .small()
                                                .color(theme::OUTLINE),
                                        );
                                    },
                                );
                            });
                            ui.label(
                                egui::RichText::new(&message.content).color(theme::ON_SURFACE),
                            );
                        });
                        ui.add_space(theme::SPACE_XS);
                    }
                    Role::Assistant => {
                        let stats = infer.stats.get(assistant_index).cloned();
                        assistant_index += 1;
                        card_frame().show(ui, |ui| {
                            ui.set_min_width(ui.available_width());
                            ui.horizontal(|ui| {
                                kit::badge(ui, "SRV", theme::PRIMARY_CONTAINER);
                                ui.label(
                                    egui::RichText::new(if infer.model.is_empty() {
                                        String::from("assistente")
                                    } else {
                                        format!("assistente ({})", infer.model)
                                    })
                                    .small()
                                    .strong()
                                    .color(theme::ON_SURFACE),
                                );
                                ui.with_layout(
                                    egui::Layout::right_to_left(egui::Align::Center),
                                    |ui| {
                                        let content = message.content.clone();
                                        if ui.small_button("Copiar").clicked() {
                                            ui.ctx().copy_text(content);
                                        }
                                        if let Some(stats) = &stats {
                                            let tokens = match (
                                                stats.prompt_tokens,
                                                stats.completion_tokens,
                                            ) {
                                                (Some(p), Some(c)) => {
                                                    format!(" · {p}+{c} tokens")
                                                }
                                                (None, Some(c)) => {
                                                    format!(" · {c} tokens de saída")
                                                }
                                                _ => String::new(),
                                            };
                                            let tps = stats
                                                .tokens_per_sec()
                                                .map(|v| format!(" · {v:.1} tok/s"))
                                                .unwrap_or_default();
                                            ui.label(
                                                egui::RichText::new(format!(
                                                    "{} · {}ms{tokens}{tps}",
                                                    hhmmss_utc(message.ts_unix_ms),
                                                    stats.elapsed_ms
                                                ))
                                                .monospace()
                                                .small()
                                                .color(theme::PRIMARY_FIXED_DIM),
                                            );
                                        } else {
                                            ui.label(
                                                egui::RichText::new(hhmmss_utc(message.ts_unix_ms))
                                                    .monospace()
                                                    .small()
                                                    .color(theme::OUTLINE),
                                            );
                                        }
                                    },
                                );
                            });
                            ui.label(
                                egui::RichText::new(&message.content).color(theme::SECONDARY_FIXED),
                            );
                        });
                        ui.add_space(theme::SPACE_XS);
                    }
                }
            }
        });
    ui.add_space(theme::SPACE_SM);

    // ── Geração em andamento (mockup; só com `busy` real) ──
    if let Some(started) = infer.pending_since {
        let elapsed = started.elapsed().as_secs_f32();
        card_frame().show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.horizontal(|ui| {
                ui.spinner();
                ui.label(
                    egui::RichText::new(format!(
                        "Geração em Andamento: {elapsed:.1}s (Timeout máx: 120s · REQ/T-820-19)"
                    ))
                    .small()
                    .strong()
                    .color(theme::ON_SURFACE),
                );
            });
            ui.label(
                egui::RichText::new(match discovery.servers.first() {
                    Some(server) => format!(
                        "Aguardando tokens… · FILA: {} idle · {} processing",
                        server.slots_idle, server.slots_processing
                    ),
                    None => String::from("Aguardando tokens… · FILA: sem ServerStatus no domínio"),
                })
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
        });
        ui.add_space(theme::SPACE_SM);
    }

    // ── Composer (mockup) ──
    let composer = ui.add(
        egui::TextEdit::multiline(&mut infer.prompt)
            .desired_rows(3)
            .desired_width(ui.available_width())
            .hint_text(
                "Digite o comando ou questão técnica para o cluster de inferência… \
                 (Ex.: Como renegociar QoS de Durability via IDL sem reiniciar nós?)",
            ),
    );
    ui.add_space(theme::SPACE_XS);
    let chars = infer.prompt.chars().count();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{chars} caracteres · ~{} tokens (estimativa ×/4)",
                chars / 4
            ))
            .monospace()
            .small()
            .color(theme::OUTLINE),
        );
        ui.label(
            egui::RichText::new("Atalho: Ctrl + Enter para enviar")
                .small()
                .color(theme::OUTLINE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let can_send = !infer.busy && !infer.prompt.trim().is_empty();
            let send = ui.add_enabled(
                can_send,
                egui::Button::new(
                    egui::RichText::new(if infer.busy {
                        "gerando…"
                    } else {
                        "▶ ENVIAR PROMPT"
                    })
                    .monospace()
                    .color(theme::ON_PRIMARY),
                )
                .fill(theme::PRIMARY_CONTAINER),
            );
            let ctrl_enter = composer.has_focus()
                && ui.input(|i| {
                    (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::Enter)
                });
            if (send.clicked() || ctrl_enter) && can_send {
                infer.send();
                crate::studio_log::info("inferência: prompt enviado ao servidor");
            }
            if ui
                .add_enabled(infer.busy, egui::Button::new("Parar Geração"))
                .clicked()
            {
                infer.cancel();
            }
        });
    });
    if !infer.reply.is_empty()
        && (infer.reply.starts_with("transcript") || infer.reply.starts_with("falha"))
    {
        ui.label(
            egui::RichText::new(&infer.reply)
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
        );
    }
}

#[cfg(test)]
mod tests {
    use super::{hhmmss_utc, session_tokens, temp_caption, temp_mode};
    use crate::inference::{InferenceState, TurnStats};

    #[test]
    fn clock_formats_utc_wall_time() {
        assert_eq!(hhmmss_utc(0), "00:00:00 UTC");
        assert_eq!(hhmmss_utc(3_661_000), "01:01:01 UTC");
        assert_eq!(hhmmss_utc(86_399_999), "23:59:59 UTC");
    }

    #[test]
    fn temperature_buckets_match_documented_ranges() {
        assert_eq!(temp_caption(0.2), "Preciso / baixo jitter");
        assert_eq!(temp_caption(0.5), "Equilibrado");
        assert_eq!(temp_caption(1.5), "Criativo / dispersivo");
        assert_eq!(temp_mode(0.0), "DETERMINÍSTICO");
        assert_eq!(temp_mode(0.2), "FOCADO");
        assert_eq!(temp_mode(1.0), "EXPLORATÓRIO");
    }

    #[test]
    fn session_tokens_sum_known_usage_only() {
        let mut infer = InferenceState::new();
        assert_eq!(session_tokens(&infer), (0, 0));
        infer.stats.push(TurnStats {
            elapsed_ms: 100,
            prompt_tokens: Some(10),
            completion_tokens: Some(5),
        });
        infer.stats.push(TurnStats {
            elapsed_ms: 50,
            prompt_tokens: None,
            completion_tokens: None,
        });
        assert_eq!(session_tokens(&infer), (15, 2));
    }
}
