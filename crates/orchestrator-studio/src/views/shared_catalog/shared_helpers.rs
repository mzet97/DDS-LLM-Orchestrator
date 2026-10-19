//! Parte de `views::shared_catalog` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}

/// Título de seção para fileiras (o `section_label` do kit consome a linha
/// toda e quebra `horizontal` — ver 3.6).
pub(super) fn header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .small()
        .strong()
        .color(theme::OUTLINE)
}

/// Idade curta de um instante ("14s"/"3m"/"2h") para Última gravação.
pub(super) fn ago(ts_ms: u64) -> String {
    let now = now_ms();
    let secs = now.saturating_sub(ts_ms.min(now)) / 1000;
    if secs < 90 {
        format!("{secs}s")
    } else if secs < 5_400 {
        format!("{}m", secs / 60)
    } else {
        format!("{}h", secs / 3600)
    }
}

pub(super) fn now_ms() -> u64 {
    crate::machines::now_unix_ns() / 1_000_000
}
