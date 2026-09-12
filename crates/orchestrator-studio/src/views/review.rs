//! Revisão de mudanças e conflitos (T12): delega ao render da lib.

use eframe::egui;
use orchestrator_studio::review::{self, ReviewState};

/// Comparação de três vias + decisões + aviso de exclusão remota.
pub fn show(ui: &mut egui::Ui, review: &mut ReviewState) {
    review::show(ui, review);
}
