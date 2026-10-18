//! Parte de `views` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_table(ui: &mut egui::Ui, dds: &mut DdsState) {
    let tools = &dds.snapshot.tools;
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
}
