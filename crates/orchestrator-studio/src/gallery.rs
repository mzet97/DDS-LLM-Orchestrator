//! Galeria de componentes (UI-1.3): mesmos tokens da aplicação,
//! exemplos de estados nos dois temas. Renderização pura sobre
//! [`crate::design`]; sem backend, sem efeitos externos.

use crate::design::{palette, ResolvedTheme};

/// Estado local da galeria (tema exibido).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct GalleryState {
    pub theme: ResolvedTheme,
}

impl GalleryState {
    #[must_use]
    pub const fn new(theme: ResolvedTheme) -> Self {
        Self { theme }
    }
}

/// Selo de estado: texto + símbolo + cor (nunca só um ponto verde).
pub fn status_badge(ui: &mut egui::Ui, symbol: &str, text: &str, color: egui::Color32) {
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new(symbol).color(color));
        ui.label(text);
    });
}

/// Linha de frescor: fonte e idade da observação.
pub fn freshness(ui: &mut egui::Ui, source: &str, age: &str) {
    ui.label(format!("Fonte: {source} · observado {age}"));
}

/// Aviso de capacidade ausente com motivo (nunca botão cinza sem texto).
pub fn capability_notice(ui: &mut egui::Ui, text: &str) {
    ui.group(|ui| {
        ui.strong("Capacidade indisponível");
        ui.label(text);
    });
}

/// Renderiza a galeria no tema do estado.
pub fn show(ui: &mut egui::Ui, state: &GalleryState) {
    let colors = palette(state.theme);
    let to_egui = |color: crate::design::Rgb| egui::Color32::from_rgb(color.0, color.1, color.2);
    ui.heading("Galeria de componentes");
    ui.label("Mesmos tokens da aplicação; alterne o tema para comparar.");
    ui.separator();
    ui.label(egui::RichText::new("Título de página 24").size(24.0));
    ui.label(egui::RichText::new("Título de seção 18").size(18.0));
    ui.label(egui::RichText::new("Corpo 15").size(15.0));
    ui.label(egui::RichText::new("Rótulo 14").size(14.0));
    ui.label(
        egui::RichText::new("Metadados 12")
            .size(12.0)
            .color(to_egui(colors.text_secondary)),
    );
    ui.monospace("IDs e caminhos em monospace 13");
    ui.separator();
    ui.horizontal(|ui| {
        // Galeria: botões sem efeito; o clique só prova foco/ativação.
        let _primary = ui.button("Ação primária");
        let _secondary = ui.button("Ação secundária");
    });
    ui.separator();
    status_badge(ui, "●", "Em execução", to_egui(colors.success_text));
    status_badge(
        ui,
        "▲",
        "Atenção: dados incompletos",
        to_egui(colors.warning_text),
    );
    status_badge(ui, "■", "Falhou", to_egui(colors.danger_text));
    freshness(ui, "cache local", "há 12 s");
    ui.separator();
    capability_notice(
        ui,
        "A interface está pronta para esta ação, mas não há serviço de implantação conectado.",
    );
    ui.separator();
    ui.group(|ui| {
        ui.strong("Saída:");
        ui.monospace("PROVA_OK");
    });
}
