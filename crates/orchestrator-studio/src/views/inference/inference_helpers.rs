//! Parte de `views::inference` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
}

/// Cabeçalho de card: título à esquerda + etiqueta à direita.
pub(super) fn card_title(ui: &mut egui::Ui, title: &str, tag: &str) {
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
pub(super) fn hhmmss_utc(ts_unix_ms: u64) -> String {
    let secs = ts_unix_ms / 1000;
    format!(
        "{:02}:{:02}:{:02} UTC",
        (secs / 3600) % 24,
        (secs / 60) % 60,
        secs % 60
    )
}

/// Legenda honesta da temperatura (faixas documentadas da GUI).
pub(super) fn temp_caption(temperature: f32) -> &'static str {
    if temperature < 0.3 {
        "Preciso / baixo jitter"
    } else if temperature < 1.0 {
        "Equilibrado"
    } else {
        "Criativo / dispersivo"
    }
}

/// Modo do mockup derivado da temperatura real (faixas da GUI).
pub(super) fn temp_mode(temperature: f32) -> &'static str {
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
pub(super) fn card_slider(
    ui: &mut egui::Ui,
    value: &mut f32,
    range: std::ops::RangeInclusive<f32>,
) {
    ui.scope(|ui| {
        ui.style_mut().visuals.widgets.inactive.bg_fill = theme::SURFACE_HIGHEST;
        ui.style_mut().spacing.slider_width = 150.0;
        ui.add(egui::Slider::new(value, range).show_value(false));
    });
}

/// Tokens conhecidos da sessão (soma do `usage` real) + nº de turnos.
pub(super) fn session_tokens(infer: &InferenceState) -> (u64, usize) {
    let mut sum = 0u64;
    for stats in &infer.stats {
        sum += stats.prompt_tokens.unwrap_or(0) + stats.completion_tokens.unwrap_or(0);
    }
    (sum, infer.stats.len())
}
