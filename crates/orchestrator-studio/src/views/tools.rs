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

/// Status canônicos do contrato para os chips de filtro (ordem do PNG).
const STATUS_FILTERS: [(i32, &str); 6] = [
    (0, "PENDING"),
    (1, "ALLOWED"),
    (3, "EXECUTING"),
    (4, "COMPLETED"),
    (2, "DENIED"),
    (5, "FAILED"),
];

pub fn show(ui: &mut egui::Ui, dds: &mut DdsState, guard: &ProtectedGuard) {
    dds.poll();
    // QoS efetiva do `ToolCall.Request` (`dds-dataspace/src/qos.rs:275`):
    // Reliable + TransientLocal KL10 — o selo do PNG é real. FORA: tópico
    // `ToolCall/Response` (o resultado volta na MESMA instância) e "CDR
    // Serializer" (sem fonte).
    panel_header(
        ui,
        &format!(
            "SEC 3.13 · DOMÍNIO {} · DDS-QoS: RELIABLE / TRANSIENT_LOCAL · REQ: ToolCall.Request",
            dds.domain
        ),
        "Ferramentas & Tool Calls",
        "Chamadas observadas no tópico ToolCall.Request: o gateway reivindica, \
         o policy-engine decide pelo nível e o resultado volta na MESMA \
         instância (at-least-once, sem tópico de resposta).",
    );

    // ── Controles: janela + observação + export + estado do interlock ──
    // FORA: GOVERNANÇA RBAC_STRICT (a política real é estática/permissiva —
    // M2) e "Pausar Captura" (sem streaming contínuo; a captura é o
    // `Observar Domínio`).
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new("JANELA:")
                .monospace()
                .small()
                .color(theme::OUTLINE),
        );
        // Opções incluem o default real (5s) + as do PNG.
        let mut picked: Option<u64> = None;
        egui::ComboBox::from_id_salt("tools_window")
            .selected_text(format!("Últimos {} segundos", dds.window_secs))
            .width(200.0)
            .show_ui(ui, |ui| {
                for secs in [5_u64, 10, 30, 60] {
                    if ui
                        .selectable_label(
                            dds.window_secs == secs,
                            format!("Últimos {secs} segundos"),
                        )
                        .clicked()
                    {
                        picked = Some(secs);
                    }
                }
            });
        if let Some(secs) = picked {
            dds.window_secs = secs;
        }
        ui.add_enabled_ui(!dds.busy, |ui| {
            // Primário ciano (padrão 3.9): sem fill parecia desabilitado.
            if ui
                .add(
                    egui::Button::new(
                        egui::RichText::new(if dds.busy {
                            "observando…"
                        } else {
                            "Observar Domínio"
                        })
                        .monospace()
                        .small()
                        .color(theme::ON_PRIMARY),
                    )
                    .fill(theme::PRIMARY_CONTAINER)
                    .corner_radius(egui::CornerRadius::same(theme::RADIUS_PILL as u8)),
                )
                .clicked()
            {
                dds.refresh();
            }
            if ui
                .add(egui::Button::new(
                    egui::RichText::new("Exportar JSON").monospace().small(),
                ))
                .clicked()
            {
                match dds.export_tools() {
                    Ok(path) => crate::studio_log::info(format!(
                        "ferramentas: {} chamada(s) exportadas para {}",
                        dds.snapshot.tools.len(),
                        path.display()
                    )),
                    Err(err) => {
                        dds.error = format!("exportação falhou: {err}");
                    }
                }
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
    // Bloqueios DO N2 (o total da janela já aparece nos filtros; o sub do
    // N2 com o total global mentia quando o DENIED era de outro nível).
    let denied_n2 = tools
        .iter()
        .filter(|t| t.security_level == 2 && t.status == 2)
        .count();
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
                if denied_n2 > 0 {
                    format!(" · {denied_n2} bloqueio(s) DENIED")
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

    // ── Tabela de auditoria (ordem do PNG; GUID do solicitante FORA) ──
    ui.label(header("AUDITORIA DE TOOL CALLS EM TEMPO REAL"));
    ui.add_space(theme::SPACE_XS);
    kit::table("tools_governance_grid").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Call ID",
                "Ferramenta",
                "Solicitante",
                "Nível de segurança",
                "Política de decisão (prévia)",
                "Status canônico",
                "Duração",
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
            // Chips (espelho do PNG) nas cores da convenção global.
            let status_color = match tool.status {
                2 | 5 => theme::ERROR,
                4 => theme::OK,
                3 => theme::WARN,
                _ => theme::PRIMARY_FIXED_DIM,
            };
            let level_color = match tool.security_level {
                0 => theme::PRIMARY_FIXED_DIM,
                1 => theme::WARN,
                2 => theme::ERROR,
                _ => theme::STALE,
            };
            let short_id = short_call_id(&tool.call_id);
            if ui
                .selectable_label(selected, egui::RichText::new(short_id).monospace())
                .on_hover_text(&tool.call_id)
                .clicked()
            {
                dds.tools_selected = Some(tool.call_id.clone());
            }
            ui.label(&tool.tool_name);
            ui.label(&tool.requester_id);
            kit::badge(
                ui,
                &crate::dds_observe::security_level_label(tool.security_level),
                level_color,
            );
            ui.label(egui::RichText::new(&tool.result_preview).small().weak());
            kit::badge(ui, status, status_color);
            kit::num_cell(
                ui,
                &if tool.duration_ms > 0 {
                    // Milhar para legibilidade; valor absurdo (ex.: wall-ms
                    // quando o produtor não preenche `created_at_ns`) é dado
                    // do fio — nunca escondido (achado 3.13).
                    format!("{}ms", fmt_int(tool.duration_ms))
                } else {
                    String::from("aberta")
                },
            );
            ui.end_row();
        }
    });
    ui.add_space(theme::SPACE_MD);

    // ── Inspetor REQUEST × RESPONSE (payloads íntegros do fio, PRD 3.13).
    // FORA: "CDR Serial", QoS Volatile/TTL e Transport (sem fonte).
    if let Some(selected_id) = dds.tools_selected.clone() {
        if let Some(tool) = tools.iter().find(|t| t.call_id == selected_id) {
            section(
                ui,
                &format!(
                    "INSPETOR DE CHAMADA SELECIONADA: {} · {}",
                    short_call_id(&tool.call_id),
                    status_label(tool.status)
                ),
            );
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
                        cols[0].label(header("PAYLOAD DE ENTRADA (arguments_json)"));
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
                        cols[1].label(header("RESULTADO (result_json)"));
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

/// Título de seção (o `section_label` do kit centraliza — achado 3.12).
fn header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .small()
        .strong()
        .color(theme::OUTLINE)
}

/// Seção com hairline, título à esquerda (espelho do PNG).
fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(header(text));
    ui.separator();
    ui.add_space(theme::SPACE_SM);
}

/// ID curto para a tabela: até 8 chars mostra tudo, acima disso os
/// ÚLTIMOS 8 (IDs reais `probe-1a…` diferem só no fim — take do início
/// colidia; `…` no meio estourou a célula comprimida e o egui elidiu o
/// sufixo — captura 3.13 + probe kittest). Íntegra no hover.
fn short_call_id(call_id: &str) -> String {
    if call_id.chars().count() <= 8 {
        return call_id.to_owned();
    }
    call_id
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

/// Inteiro com separador de milhar (legibilidade de durações grandes).
fn fmt_int(n: u64) -> String {
    let digits = n.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}

#[cfg(test)]
mod tests {
    use super::{fmt_int, short_call_id};

    #[test]
    fn fmt_int_groups_thousands() {
        assert_eq!(fmt_int(840), "840");
        assert_eq!(fmt_int(1_791_257_507_167), "1,791,257,507,167");
    }

    #[test]
    fn short_call_id_keeps_short_and_clips_long() {
        assert_eq!(short_call_id("aa-1111"), "aa-1111");
        assert_eq!(short_call_id("aa-11111111-0001"), "111-0001");
        assert_eq!(short_call_id("probe-1a10f44542a"), "0f44542a");
        assert_eq!(short_call_id("probe-1a10f447561"), "0f447561");
    }
}
