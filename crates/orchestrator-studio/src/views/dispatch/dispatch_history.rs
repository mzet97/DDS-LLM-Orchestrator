//! Parte de `views::dispatch` (extraída para manter os arquivos revisáveis).

/// Rodapé: histórico da sessão (últimos 5) com taxa e média.
use super::*;

pub(super) fn history_section(ui: &mut egui::Ui, dispatch: &mut DispatchState) {
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
pub(super) fn hhmmss_mmm(ts_unix_ms: u64) -> String {
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
pub(super) fn short_id(task_id: &str) -> String {
    if task_id.chars().count() > 12 {
        format!("{}…", task_id.chars().take(12).collect::<String>())
    } else {
        String::from(task_id)
    }
}
