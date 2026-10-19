//! Parte de `views` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_stream(
    ui: &mut egui::Ui,
    panel: &mut LogsPanel,
    live: &[LogEntry],
    rate: Option<f64>,
    retention: &str,
    evicted: u64,
) {
    // Faixa de stream (espelho do PNG; tudo medido do buffer).
    ui.horizontal(|ui| {
        kit::badge(ui, "FIFO RING BUFFER [500]", theme::PRIMARY_FIXED_DIM);
        ui.label(
            egui::RichText::new(format!(
                "STREAM: {} · Taxa {} · RETENÇÃO: {retention}",
                if panel.paused {
                    "PAUSADO (foto)"
                } else {
                    "REALTIME ATIVO"
                },
                rate.map_or(String::from("—"), |r| format!("~{r:.1} logs/s")),
            ))
            .monospace()
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
    });
    ui.add_space(theme::SPACE_XS);

    // ── 4 KPIs ──
    let warn_count = live.iter().filter(|e| e.level == "WARN").count();
    let error_count = live.iter().filter(|e| e.level == "ERRO").count();
    let pct = live.len() * 100 / studio_log::CAPACITY;
    let share = |n: usize| {
        if live.is_empty() {
            0.0
        } else {
            n as f64 * 100.0 / live.len() as f64
        }
    };
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Ocupação do ring buffer",
            format!("{}/500", live.len()),
            &format!("{pct}% cheio · {evicted} descartados"),
            theme::PRIMARY_FIXED_DIM,
        );
        absorption_card(&mut cols[1], rate, live);
        kit::metric_card(
            &mut cols[2],
            "Severidade de alerta",
            // Valor HERO curto (achado 3.7: "N Falhas Ativas" quebra).
            format!("{error_count}"),
            &format!(
                "Falhas Ativas · {:.1}% ERRO · {:.1}% WARN",
                share(error_count),
                share(warn_count)
            ),
            if error_count > 0 {
                theme::ERROR
            } else if warn_count > 0 {
                theme::WARN
            } else {
                theme::OK
            },
        );
        kit::metric_card(
            &mut cols[3],
            "Sink stderr / TTY",
            String::from("SINC"),
            &if evicted == 0 {
                String::from("/dev/stderr · SEM DROP")
            } else {
                format!("/dev/stderr · {evicted} descartados")
            },
            theme::OK,
        );
    });
    ui.add_space(theme::SPACE_MD);
}
