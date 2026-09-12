//! Painel de ambientes (T01): recentes, conhecidos, informar endereço
//! e demonstração. Encontrar um ambiente não inicia serviços; entrar em
//! demonstração rotula a fonte sem tocar dados reais.

use eframe::egui;
use orchestrator_studio::nodes::NodeRegistry;
use orchestrator_studio::shell::{DataSource, ShellContext};

/// Abertura e retomada de ambientes.
pub fn show(ui: &mut egui::Ui, registry: &NodeRegistry, shell: &mut ShellContext) {
    ui.heading("Ambientes");
    ui.label("Continue um projeto existente sem cadastrar recursos novamente.");
    if registry.list().is_empty() {
        ui.label(
            "Nenhum ambiente conhecido. Informe o endereço na tela do nó ou entre em demonstração.",
        );
    } else {
        ui.strong("Recentes e conhecidos:");
        for entry in registry.list() {
            ui.monospace(format!("{} · {}", entry.alias, entry.url));
        }
    }
    ui.separator();
    if ui.button("Entrar em demonstração").clicked() {
        shell.source = DataSource::Demo;
    }
    if shell.source == DataSource::Demo {
        ui.group(|ui| {
            ui.strong("Demonstração · dados simulados");
            ui.label("Nenhuma máquina será alterada. Cache separado do modo real.");
        });
    }
}
