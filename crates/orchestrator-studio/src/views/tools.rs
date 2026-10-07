//! Painel 3.13 Ferramentas & Tool Calls (T-890-06, G-18..22): governança ao
//! vivo do `ToolCall.Request` — cards de nível de segurança no vocabulário
//! do PRD (N0 READ_ONLY / N1 SANDBOX_EXEC / N2 HOST_MUTATION), chips de
//! filtro por status canônico com contagem + taxa REQ/s + bloqueios da
//! sessão, tabela com requester/nível/duração e inspetor REQUEST×RESPONSE
//! lado a lado (payloads íntegros do fio). Usa o MESMO `DdsState` da
//! Topologia.

use crate::dds_observe::{status_label, DdsState};
use crate::kit;
use crate::panel_header::panel_header;
use crate::protected::ProtectedGuard;
use crate::theme;
use eframe::egui;

/// Status canônicos do contrato para os chips de filtro.
const STATUS_FILTERS: [(i32, &str); 6] = [
    (0, "PENDING"),
    (1, "ALLOWED"),
    (2, "DENIED"),
    (3, "EXECUTING"),
    (4, "COMPLETED"),
    (5, "FAILED"),
];

pub fn show(ui: &mut egui::Ui, dds: &mut DdsState, guard: &ProtectedGuard) {
    dds.poll();
    panel_header(
        ui,
        &format!(
            "SEC 3.13 · FERRAMENTAS & TOOL CALLS · DOMÍNIO {} · REQ: ToolCall.Request",
            dds.domain
        ),
        "Ferramentas",
        "Chamadas observadas no tópico ToolCall.Request: o gateway reivindica, \
         o policy-engine decide pelo nível e o resultado volta na MESMA \
         instância (at-least-once, sem tópico de resposta).",
    );

    // ── Controles: janela + observação + estado do interlock ──
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("JANELA:")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        for secs in [10_u64, 30, 60] {
            let active = dds.window_secs == secs;
            if ui
                .selectable_label(
                    active,
                    egui::RichText::new(format!("{secs}s")).monospace().small(),
                )
                .clicked()
            {
                dds.window_secs = secs;
            }
        }
        ui.add_enabled_ui(!dds.busy, |ui| {
            if ui
                .add(egui::Button::new(
                    egui::RichText::new(if dds.busy {
                        "observando…"
                    } else {
                        "Observar Domínio"
                    })
                    .monospace()
                    .small()
                    .color(theme::ON_PRIMARY),
                ))
                .clicked()
            {
                dds.refresh();
            }
        });
        kit::badge(
            ui,
            if guard.armed {
                "INTERLOCK: ARMADO"
            } else {
                "INTERLOCK: DESARMADO"
            },
            if guard.armed {
                theme::ERROR
            } else {
                theme::STALE
            },
        );
    });
    if !dds.error.is_empty() && !dds.busy {
        kit::error_banner(ui, &dds.error);
    }
    ui.add_space(theme::SPACE_MD);

    // ── 3 cards de nível (vocabulário PRD 3.13) com contagens reais ──
    let tools = &dds.snapshot.tools;
    let level_count = |level: i32| tools.iter().filter(|t| t.security_level == level).count();
    let higher = tools.iter().filter(|t| t.security_level > 2).count();
    let denied_total = tools.iter().filter(|t| t.status == 2).count();
    ui.columns(3, |cols| {
        kit::metric_card(
            &mut cols[0],
            "N0 READ_ONLY",
            level_count(0).to_string(),
            "leitura pura — sem efeito no host",
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "N1 SANDBOX_EXEC",
            level_count(1).to_string(),
            "execução isolada (sandbox)",
            theme::WARN,
        );
        kit::metric_card(
            &mut cols[2],
            "N2 HOST_MUTATION",
            level_count(2).to_string(),
            &format!(
                "mutação no host · {} nível(is) fora da escala{}",
                higher,
                if denied_total > 0 {
                    format!(" · {denied_total} bloqueio(s) DENIED")
                } else {
                    String::new()
                }
            ),
            theme::ERROR,
        );
    });
    ui.add_space(theme::SPACE_MD);

    if tools.is_empty() {
        kit::empty_state(
            ui,
            "Nenhuma tool call na janela. Suba mcp-gateway + policy-engine no \
             domínio e gere uma chamada (ex.: um agente com engine LLM que \
             emita tool_call).",
        );
        return;
    }

    // ── Chips de status + taxa REQ/s da janela + bloqueios da sessão ──
    // (taxa honesta: chamadas drenadas ÷ janela — não é QPS do barramento)
    let rate = tools.len() as f64 / dds.window_secs.max(1) as f64;
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(
                dds.tools_filter.is_none(),
                egui::RichText::new(format!("Todos ({})", tools.len()))
                    .monospace()
                    .small(),
            )
            .clicked()
        {
            dds.tools_filter = None;
        }
        for (code, name) in STATUS_FILTERS {
            let count = tools.iter().filter(|t| t.status == code).count();
            if ui
                .selectable_label(
                    dds.tools_filter == Some(code),
                    egui::RichText::new(format!("{name} ({count})"))
                        .monospace()
                        .small(),
                )
                .clicked()
            {
                dds.tools_filter = Some(code);
            }
        }
        ui.separator();
        ui.label(
            egui::RichText::new(format!("TAXA {rate:.1} REQ/s (janela)"))
                .monospace()
                .small()
                .color(theme::PRIMARY_FIXED_DIM),
        );
        ui.separator();
        ui.label(
            egui::RichText::new(format!("BLOQUEIOS (DENIED na janela): {denied_total}"))
                .monospace()
                .small()
                .color(if denied_total > 0 {
                    theme::ERROR
                } else {
                    theme::OK
                }),
        );
    });
    ui.add_space(theme::SPACE_SM);

    // ── Tabela de auditoria (política de decisão = prévia do resultado) ──
    egui::Grid::new("tools_governance_grid")
        .striped(true)
        .show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "Call ID",
                    "Ferramenta",
                    "Solicitante",
                    "Nível",
                    "Status canônico",
                    "Duração",
                    "Política de decisão (prévia)",
                ],
            );
            for tool in tools {
                if dds.tools_filter.is_some_and(|code| tool.status != code) {
                    continue;
                }
                let selected = dds
                    .tools_selected
                    .as_deref()
                    .is_some_and(|id| id == tool.call_id);
                let status = status_label(tool.status);
                let status_color = match tool.status {
                    2 | 5 => theme::ERROR,
                    4 => theme::OK,
                    3 => theme::WARN,
                    _ => theme::ON_SURFACE_VARIANT,
                };
                let short_id: String = tool.call_id.chars().take(8).collect();
                if ui
                    .selectable_label(selected, egui::RichText::new(short_id).monospace())
                    .clicked()
                {
                    dds.tools_selected = Some(tool.call_id.clone());
                }
                ui.label(&tool.tool_name);
                ui.label(&tool.requester_id);
                kit::mono_cell(
                    ui,
                    &crate::dds_observe::security_level_label(tool.security_level),
                );
                ui.label(egui::RichText::new(status).monospace().color(status_color));
                kit::num_cell(
                    ui,
                    &if tool.duration_ms > 0 {
                        format!("{} ms", tool.duration_ms)
                    } else {
                        String::from("aberta")
                    },
                );
                ui.label(egui::RichText::new(&tool.result_preview).small().weak());
                ui.end_row();
            }
        });
    ui.add_space(theme::SPACE_MD);

    // ── Inspetor REQUEST × RESPONSE (payloads íntegros do fio, PRD 3.13) ──
    if let Some(selected_id) = dds.tools_selected.clone() {
        if let Some(tool) = tools.iter().find(|t| t.call_id == selected_id) {
            kit::section_label(ui, "INSPETOR DE CHAMADA · REQUEST × RESPONSE");
            egui::Frame::NONE
                .fill(theme::SURFACE_LOW)
                .corner_radius(egui::CornerRadius::same(theme::RADIUS_SM as u8))
                .inner_margin(theme::SPACE_MD)
                .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
                .show(ui, |ui| {
                    ui.set_min_width(ui.available_width());
                    let level = crate::dds_observe::security_level_label(tool.security_level);
                    let raw_json = serde_json::json!({
                        "call_id": tool.call_id,
                        "tool_name": tool.tool_name,
                        "requester_id": tool.requester_id,
                        "security_level": level,
                        "status": status_label(tool.status),
                        "duration_ms": tool.duration_ms,
                        "arguments": serde_json::from_str::<serde_json::Value>(
                            &tool.arguments_json
                        )
                        .unwrap_or(serde_json::Value::String(tool.arguments_json.clone())),
                        "result": serde_json::from_str::<serde_json::Value>(&tool.result_json)
                            .unwrap_or(serde_json::Value::String(tool.result_json.clone())),
                    });
                    ui.horizontal(|ui| {
                        ui.label(
                            egui::RichText::new(format!(
                                "● {} · {} · {}",
                                tool.tool_name,
                                level,
                                status_label(tool.status)
                            ))
                            .monospace()
                            .small()
                            .color(theme::PRIMARY_FIXED_DIM),
                        );
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new("COPIAR RAW").monospace().small(),
                            ))
                            .clicked()
                        {
                            ui.ctx().copy_text(
                                serde_json::to_string_pretty(&raw_json).unwrap_or_default(),
                            );
                            crate::studio_log::info(format!(
                                "ferramentas: chamada {} copiada como JSON",
                                tool.call_id
                            ));
                        }
                    });
                    ui.columns(2, |cols| {
                        kit::section_label(&mut cols[0], "REQUEST (arguments_json)");
                        let args = if tool.arguments_json.is_empty() {
                            String::from("(vazio no fio)")
                        } else {
                            tool.arguments_json.clone()
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("tool-request")
                            .max_height(220.0)
                            .show(&mut cols[0], |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut args.clone())
                                        .font(egui::TextStyle::Monospace)
                                        .desired_rows(6)
                                        .interactive(false),
                                );
                            });
                        kit::section_label(&mut cols[1], "RESPONSE (result_json)");
                        let result = if tool.result_json.is_empty() {
                            String::from("(ainda sem resultado)")
                        } else {
                            tool.result_json.clone()
                        };
                        egui::ScrollArea::vertical()
                            .id_salt("tool-response")
                            .max_height(220.0)
                            .show(&mut cols[1], |ui| {
                                ui.add(
                                    egui::TextEdit::multiline(&mut result.clone())
                                        .font(egui::TextStyle::Monospace)
                                        .desired_rows(6)
                                        .interactive(false),
                                );
                            });
                    });
                });
        }
    }
}
