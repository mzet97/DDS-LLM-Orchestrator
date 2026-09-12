//! Demonstração (UI-5.1): cenários selecionáveis sobre fixtures
//! rotuladas. Entrar aqui não toca dados reais nem inicia serviços.

use eframe::egui;
use orchestrator_studio::preview::{demo_banner, FixtureSet, PreviewScenario};

/// Estado local da tela de demonstração.
#[derive(Debug, Clone)]
pub struct PreviewView {
    pub scenario: PreviewScenario,
    pub fixtures: FixtureSet,
}

impl PreviewView {
    pub fn new() -> Self {
        Self {
            scenario: PreviewScenario::ExistingEnvironment,
            fixtures: FixtureSet::standard(),
        }
    }
}

impl Default for PreviewView {
    fn default() -> Self {
        Self::new()
    }
}

/// Cenários + fixtures + selo persistente de simulação.
pub fn show(ui: &mut egui::Ui, view: &mut PreviewView) {
    ui.heading("Demonstração");
    ui.group(|ui| {
        ui.strong(demo_banner());
    });
    ui.label("cenário:");
    egui::ComboBox::from_label("")
        .selected_text(view.scenario.label())
        .show_ui(ui, |ui| {
            for scenario in PreviewScenario::all() {
                ui.selectable_value(&mut view.scenario, *scenario, scenario.label());
            }
        });
    ui.label(view.scenario.expected());
    ui.separator();
    egui::Grid::new("preview_fixtures").show(ui, |ui| {
        ui.label("id");
        ui.label("nome");
        ui.label("tipo");
        ui.label("estado");
        ui.end_row();
        for item in &view.fixtures.resources {
            ui.monospace(&item.id);
            ui.label(&item.name);
            ui.label(&item.kind);
            ui.label(&item.state);
            ui.end_row();
        }
    });
}
