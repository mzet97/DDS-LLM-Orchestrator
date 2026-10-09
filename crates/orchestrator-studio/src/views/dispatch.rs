//! Painel 3.6 Despacho (DIAG-3.6): teste síncrono direto ao orquestrador
//! (`POST /api/v1/chat/completions/sync`) — parâmetros reais à esquerda,
//! card de resultado com decomposição T1–T6 à direita e histórico da sessão.
//!
//! Só dados reais: temperatura/max_tokens/timeout vão de verdade no fio;
//! stop-sequence, tok/s, estratégia e fila global do mockup NÃO existem no
//! contrato (o backend zera `tokens_*` e não tem `stop`) e ficam FORA.

use crate::discovery::DiscoveryState;
use crate::kit;
use crate::panel_header::panel_header;
use crate::theme;
use crate::workload::{DispatchOutcome, DispatchState};
use eframe::egui;

pub fn show(ui: &mut egui::Ui, dispatch: &mut DispatchState, discovery: &DiscoveryState) {
    panel_header(
        ui,
        "DIAG-3.6",
        "3.6 Despacho de Tarefas",
        "Teste Síncrono Direto contra Orquestrador HTTP · Bypass de Workflows para Calibração de Nó",
    );
    ui.label(
        egui::RichText::new("● POST /api/v1/chat/completions/sync")
            .monospace()
            .small()
            .color(theme::PRIMARY_FIXED_DIM),
    );
    ui.add_space(theme::SPACE_XS);

    // Drena o worker de despacho (thread + mpsc — REQ/T-820-19).
    dispatch.poll();

    // ── Chips de estado + RESET à direita ──
    ui.horizontal(|ui| {
        let (conn_text, conn_color) = conn_chip(dispatch);
        kit::badge(ui, &conn_text, conn_color);
        let pool = discovery.agents.len();
        kit::badge(
            ui,
            &if pool == 1 {
                String::from("1 Agente no Pool")
            } else {
                format!("{pool} Agentes no Pool")
            },
            if pool == 0 {
                theme::STALE
            } else {
                theme::PRIMARY_FIXED_DIM
            },
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("↻ RESET DE DIAGNÓSTICO").clicked() {
                dispatch.reset();
            }
        });
    });
    ui.add_space(theme::SPACE_SM);

    ui.columns(2, |columns| {
        params_column(&mut columns[0], dispatch, discovery);
        result_column(&mut columns[1], dispatch);
    });

    ui.add_space(theme::SPACE_MD);
    history_section(ui, dispatch);
}

/// Chip de conexão: último estado CONHECIDO do orquestrador (nunca sonda
/// sozinho — só reflete despachos que o operador pediu).
fn conn_chip(dispatch: &DispatchState) -> (String, egui::Color32) {
    if dispatch.busy {
        return (String::from("○ DESPACHANDO…"), theme::WARN);
    }
    match &dispatch.last {
        Some(DispatchOutcome::Completed { .. }) => {
            let port = dispatch
                .url
                .rsplit(':')
                .next()
                .filter(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));
            let text = match port {
                Some(digits) => format!("● CONECTADO · :{digits}"),
                None => String::from("● CONECTADO"),
            };
            (text, theme::OK)
        }
        Some(DispatchOutcome::Failed { .. }) => (String::from("× FALHA NO BACKEND"), theme::ERROR),
        None if dispatch.result.starts_with("erro de despacho") => {
            (String::from("× ORQUESTRADOR INALCANÇÁVEL"), theme::ERROR)
        }
        None if !dispatch.result.is_empty() => {
            (String::from("○ ÚLTIMO ENVIO COM ERRO"), theme::WARN)
        }
        None => (String::from("○ NUNCA TESTADO"), theme::STALE),
    }
}

/// Coluna esquerda: parâmetros reais do despacho.
fn params_column(ui: &mut egui::Ui, dispatch: &mut DispatchState, discovery: &DiscoveryState) {
    // section_label consome a linha toda — em fileiras usa-se rótulo plano.
    ui.horizontal(|ui| {
        ui.label(header("▶ PARÂMETROS DE DESPACHO"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("HTTP POST DIRETO").small().weak());
        });
    });
    ui.add_space(theme::SPACE_XS);

    ui.label(caption("ENDPOINT DO ORQUESTRADOR"));
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("URI:").monospace().small());
        ui.add(
            egui::TextEdit::singleline(&mut dispatch.url)
                .desired_width(f32::INFINITY)
                .hint_text("http://127.0.0.1:8085"),
        );
    });
    ui.label(
        egui::RichText::new("/api/v1/chat/completions/sync (caminho fixo no fio)")
            .monospace()
            .small()
            .weak(),
    );
    ui.add_space(theme::SPACE_XS);

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(caption("SELETOR DE AGENTE / MODELO"));
            agent_combo(ui, dispatch, discovery);
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label(caption("MODO DE EXECUÇÃO"));
            ui.label(
                egui::RichText::new("● Síncrono (Wait-for-completion)")
                    .color(theme::PRIMARY_FIXED_DIM),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);

    // Trio numérico real (stop-sequence do mockup não existe no backend).
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(caption("TIMEOUT (MS)"));
            ui.add(
                egui::TextEdit::singleline(&mut dispatch.timeout_ms)
                    .desired_width(90.0)
                    .hint_text("300000"),
            );
        });
        ui.vertical(|ui| {
            ui.label(caption("TEMPERATURE"));
            ui.add(
                egui::TextEdit::singleline(&mut dispatch.temperature)
                    .desired_width(70.0)
                    .hint_text("0.7"),
            );
        });
        ui.vertical(|ui| {
            ui.label(caption("MAX TOKENS"));
            ui.add(
                egui::TextEdit::singleline(&mut dispatch.max_tokens)
                    .desired_width(70.0)
                    .hint_text("256"),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);

    ui.horizontal(|ui| {
        ui.label(caption("PAYLOAD DE PROMPT / INSTRUÇÃO TÉCNICA"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new("[CTRL + ENTER PARA ENVIAR]")
                    .monospace()
                    .small()
                    .weak(),
            );
        });
    });
    ui.add(
        egui::TextEdit::multiline(&mut dispatch.prompt)
            .desired_rows(8)
            .desired_width(f32::INFINITY)
            .hint_text("ex.: resuma o estado do nó alvo em 3 bullets"),
    );
    // Ctrl+Enter despacha (o multiline não consome o atalho).
    if ui.input(|state| state.modifiers.ctrl && state.key_pressed(egui::Key::Enter))
        && !dispatch.busy
        && !dispatch.prompt.trim().is_empty()
    {
        dispatch.send();
    }
    ui.add_space(theme::SPACE_XS);

    ui.horizontal(|ui| {
        let send = ui.add_enabled(
            !dispatch.busy && !dispatch.prompt.trim().is_empty(),
            egui::Button::new(
                egui::RichText::new(if dispatch.busy {
                    "○ despachando… (agente real infere)"
                } else {
                    "▶ Despachar Task Síncrona"
                })
                .monospace()
                .color(theme::ON_PRIMARY),
            )
            .fill(theme::PRIMARY_CONTAINER),
        );
        if send.clicked() {
            dispatch.send();
        }
        if ui.button("Limpar").clicked() {
            dispatch.prompt.clear();
        }
    });

    // Erros de validação/rede aparecem SÓ aqui (o card da direita cobre o
    // desfecho estruturado; "falha {id}" pertence ao card, não ao banner).
    if dispatch.result.contains("inválid") || dispatch.result.starts_with("erro de despacho") {
        ui.add_space(theme::SPACE_XS);
        kit::error_banner(ui, &dispatch.result);
    }
}

/// Combo de agentes vivos (DDS): escolher preenche o modelo efetivo; sem
/// agentes, o campo de texto abaixo segue editável (modelo manual).
fn agent_combo(ui: &mut egui::Ui, dispatch: &mut DispatchState, discovery: &DiscoveryState) {
    let live = &discovery.agents;
    egui::ComboBox::from_id_salt("dispatch_agent_model")
        .selected_text(if live.is_empty() {
            String::from("nenhum agente no domínio — modelo manual")
        } else {
            format!("{} agente(s) no domínio — escolher", live.len())
        })
        .show_ui(ui, |ui| {
            for agent in live {
                ui.selectable_value(
                    &mut dispatch.model,
                    agent.model.clone(),
                    format!("{} ({})", agent.agent_id, agent.model),
                );
            }
        });
    ui.add(
        egui::TextEdit::singleline(&mut dispatch.model)
            .desired_width(220.0)
            .hint_text("modelo efetivo"),
    );
}

fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}

/// Título de seção para fileiras (o `section_label` do kit consome a linha
/// toda e quebra `horizontal`: some badge, corta o bloco direito).
fn header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .small()
        .strong()
        .color(theme::OUTLINE)
}

/// Coluna direita: card do resultado estruturado.
fn result_column(ui: &mut egui::Ui, dispatch: &mut DispatchState) {
    // Aba em local (o `match` abaixo toma `&dispatch.last` emprestado).
    let mut tab = dispatch.selected_tab.min(2);
    match &dispatch.last {
        Some(DispatchOutcome::Completed {
            task_id,
            assigned_agent,
            latency_ms,
            content,
            breakdown,
            http_status,
            raw,
            headers,
        }) => {
            let status_text = if *http_status == 200 {
                String::from("200 OK")
            } else {
                http_status.to_string()
            };
            kit::accent_card(ui, theme::OK, |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!("● TAREFA CONCLUÍDA (Status: {status_text})"))
                            .color(theme::OK)
                            .strong(),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        kit::mono_cell(ui, &format!("ID: {task_id}"));
                    });
                });
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.vertical(|ui| {
                        ui.label(caption("AGENTE EXECUTOR"));
                        ui.label(
                            egui::RichText::new(
                                assigned_agent.as_deref().unwrap_or("não informado"),
                            )
                            .strong()
                            .color(theme::PRIMARY_FIXED_DIM),
                        );
                    });
                    ui.separator();
                    ui.vertical(|ui| {
                        ui.label(caption("LATÊNCIA TOTAL"));
                        ui.label(egui::RichText::new(format!("{latency_ms} ms")).strong());
                        if let Some(b) = breakdown {
                            ui.label(
                                egui::RichText::new(format!(
                                    "Fila: {} ms | Inf: {} ms",
                                    b.queue_ms, b.inference_ms
                                ))
                                .small()
                                .weak(),
                            );
                        }
                    });
                });
                // Decomposição real T1–T6 do `/sync`: fila × geração ×
                // transporte × serialização (âncora kittest_ux2 — formato
                // exato preservado).
                match breakdown {
                    Some(b) => {
                        ui.add_space(theme::SPACE_XS);
                        ui.label(
                            egui::RichText::new(format!(
                                "TOTAL {latency_ms} ms = FILA {} + GERAÇÃO {} + TRANSPORTE {} + SERIAL {}",
                                b.queue_ms, b.inference_ms, b.transport_ms, b.serial_ms
                            ))
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                        );
                        let parts = [
                            (b.queue_ms, theme::STALE),
                            (b.inference_ms, theme::PRIMARY_CONTAINER),
                            (b.transport_ms, theme::WARN),
                            (b.serial_ms, theme::AUTH),
                        ];
                        let sum: u64 = parts.iter().map(|(value, _)| *value).sum::<u64>().max(1);
                        ui.horizontal(|ui| {
                            for (value, color) in parts {
                                let fraction = value as f32 / sum as f32;
                                if fraction > 0.0 {
                                    ui.add(
                                        egui::ProgressBar::new(fraction)
                                            .desired_width(fraction * 480.0)
                                            .fill(color),
                                    );
                                }
                            }
                        });
                        ui.label(
                            egui::RichText::new(
                                "fila = t_agent_queue · geração = t_inference · transporte = \
                                 send+return · serial = serialize+deserialize",
                            )
                            .small()
                            .weak(),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new("decomposição T1–T6 não reportada nesta resposta")
                                .small()
                                .weak(),
                        );
                    }
                }
                ui.add_space(theme::SPACE_XS);
                ui.horizontal(|ui| {
                    ui.selectable_value(&mut tab, 0, "Resposta Formatada");
                    ui.selectable_value(&mut tab, 1, "Payload JSON Bruto");
                    ui.selectable_value(&mut tab, 2, "Headers & Telemetria");
                });
                let body = match tab {
                    0 => content
                        .as_deref()
                        .filter(|text| !text.is_empty())
                        .unwrap_or("— (backend não retornou conteúdo)")
                        .to_owned(),
                    1 => raw.clone(),
                    _ => format!("HTTP {http_status}\n{headers}"),
                };
                ui.horizontal(|ui| {
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui.button("Copiar").clicked() {
                            ui.ctx().copy_text(body.clone());
                        }
                    });
                });
                egui::ScrollArea::vertical()
                    .max_height(260.0)
                    .show(ui, |ui| {
                        ui.label(egui::RichText::new(&body).monospace().small());
                    });
            });
        }
        Some(DispatchOutcome::Failed { task_id, error }) => {
            kit::accent_card(ui, theme::ERROR, |ui| {
                ui.horizontal(|ui| {
                    ui.label(egui::RichText::new("× TAREFA FALHOU").color(theme::ERROR));
                    kit::mono_cell(ui, task_id);
                });
                ui.label(
                    egui::RichText::new(format!("motivo do backend: {error}"))
                        .small()
                        .color(theme::ERROR),
                );
            });
        }
        None => {
            // Cabeçalho próprio: equilibra a coluna com os parâmetros.
            ui.label(header("○ RESULTADO DO DESPACHO"));
            ui.add_space(theme::SPACE_XS);
            if dispatch.busy {
                kit::badge(ui, "○ DESPACHO EM ANDAMENTO", theme::WARN);
            } else {
                kit::empty_state(ui, "nenhum despacho nesta sessão ainda.");
            }
        }
    }
    dispatch.selected_tab = tab;
}

/// Rodapé: histórico da sessão (últimos 5) com taxa e média.
fn history_section(ui: &mut egui::Ui, dispatch: &mut DispatchState) {
    let total = dispatch.history.len();
    let ok_count = dispatch.history.iter().filter(|r| r.ok).count();
    let avg = {
        let latencies: Vec<u64> = dispatch
            .history
            .iter()
            .filter_map(|r| r.latency_ms)
            .collect();
        if latencies.is_empty() {
            String::from("—")
        } else {
            format!(
                "{} ms",
                latencies.iter().sum::<u64>() / latencies.len() as u64
            )
        }
    };
    ui.horizontal(|ui| {
        ui.label(header("↻ HISTÓRICO RECENTE DE DESPACHOS SÍNCRONOS"));
        kit::badge(ui, "Últimos 5 envios", theme::STALE);
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!(
                    "TAXA DE SUCESSO: {}%   MÉDIA LATÊNCIA: {avg}",
                    ok_count * 100 / total.max(1),
                ))
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
            );
        });
    });
    if dispatch.history.is_empty() {
        kit::empty_state(ui, "o histórico aparece aqui após o primeiro envio.");
        return;
    }
    let mut resend: Option<usize> = None;
    kit::table("dispatch_history").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Timestamp",
                "Task ID",
                "Modelo Alvo",
                "Agente Executor",
                "Latência",
                "Status",
                "Ação",
            ],
        );
        let len = dispatch.history.len();
        for (pos, record) in dispatch.history.iter().rev().take(5).enumerate() {
            let index = len - 1 - pos;
            kit::mono_cell(ui, &hhmmss_mmm(record.timestamp_ms));
            if record.task_id.is_empty() {
                ui.label("—");
            } else {
                ui.label(egui::RichText::new(short_id(&record.task_id)).monospace())
                    .on_hover_text(&record.task_id);
            }
            ui.label(&record.model);
            ui.label(record.agent.as_deref().unwrap_or("—"));
            kit::num_cell(
                ui,
                &record
                    .latency_ms
                    .map(|ms| format!("{ms} ms"))
                    .unwrap_or_else(|| "—".to_owned()),
            );
            if record.ok {
                ui.label(egui::RichText::new("200 OK").small().color(theme::OK));
            } else {
                ui.label(egui::RichText::new("falha").small().color(theme::ERROR))
                    .on_hover_text(&record.detail);
            }
            if ui.button("Re-enviar").clicked() {
                resend = Some(index);
            }
            ui.end_row();
        }
    });
    if let Some(index) = resend {
        dispatch.resend(index);
    }
}

/// ms unix → HH:MM:SS.mmm (conta manual, sem dependência de relógio).
fn hhmmss_mmm(ts_unix_ms: u64) -> String {
    let secs = ts_unix_ms / 1000;
    format!(
        "{:02}:{:02}:{:02}.{:03}",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60,
        ts_unix_ms % 1000,
    )
}

/// Task ID curto para a tabela (12 chars + …; ASCII do backend).
fn short_id(task_id: &str) -> String {
    if task_id.chars().count() > 12 {
        format!("{}…", task_id.chars().take(12).collect::<String>())
    } else {
        String::from(task_id)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn clock_formats_with_millis() {
        assert_eq!(hhmmss_mmm(0), "00:00:00.000");
        assert_eq!(
            hhmmss_mmm(14 * 3_600_000 + 22 * 60_000 + 1_000 + 892),
            "14:22:01.892"
        );
        assert_eq!(hhmmss_mmm(86_400_000 + 5), "00:00:00.005");
    }

    #[test]
    fn short_id_truncates_long_ids() {
        assert_eq!(short_id("t-1"), "t-1");
        assert_eq!(short_id("tsk_9c82f1b4a09"), "tsk_9c82f1b4…");
    }

    #[test]
    fn conn_chip_never_probes() {
        let idle = DispatchState::new();
        assert_eq!(conn_chip(&idle).0, "○ NUNCA TESTADO");
        let mut unreachable = DispatchState::new();
        unreachable.result = String::from("erro de despacho: x");
        assert_eq!(conn_chip(&unreachable).0, "× ORQUESTRADOR INALCANÇÁVEL");
        let mut connected = DispatchState::new();
        connected.url = String::from("http://192.168.1.61:8080");
        connected.last = Some(DispatchOutcome::Failed {
            task_id: String::from("t"),
            error: String::from("e"),
        });
        assert_eq!(conn_chip(&connected).0, "× FALHA NO BACKEND");
    }
}
