//! Painel de agentes (T04): abas separadas para **Definições** e
//! **Instâncias**. Definição ≠ processo ativo; instância sem vínculo
//! comprovado mostra "Não confirmado".

use eframe::egui;
use orchestrator_studio::agent_defs::DefinitionStore;
use orchestrator_studio::agents::AgentsState;

/// Aba ativa do painel.
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum AgentsTab {
    #[default]
    Definitions,
    Instances,
}

/// Definições locais + instâncias vivas do orquestrador.
pub fn show(
    ui: &mut egui::Ui,
    agents: &mut AgentsState,
    store: &mut DefinitionStore,
    tab: &mut AgentsTab,
) {
    ui.heading("Agentes");
    ui.horizontal(|ui| {
        ui.selectable_value(tab, AgentsTab::Definitions, "Definições");
        ui.selectable_value(tab, AgentsTab::Instances, "Instâncias");
    });
    match tab {
        AgentsTab::Definitions => show_definitions(ui, store),
        AgentsTab::Instances => show_instances(ui, agents, store),
    }
}

fn show_definitions(ui: &mut egui::Ui, store: &mut DefinitionStore) {
    if ui.button("Nova definição").clicked() {
        store.create_draft();
    }
    if store.list().is_empty() {
        ui.label("Nenhuma definição. Crie um rascunho para começar.");
        return;
    }
    for definition in store.list() {
        ui.group(|ui| {
            ui.monospace(format!(
                "{} · revisão {} {}",
                definition.id,
                definition.revision,
                if definition.published {
                    "publicada"
                } else {
                    "rascunho"
                }
            ));
            ui.label(definition.destination_summary());
        });
    }
    ui.label("Duplicar cria rascunho com nova identidade; edição completa no T05 (UI-3).");
}

fn show_instances(ui: &mut egui::Ui, agents: &mut AgentsState, store: &DefinitionStore) {
    ui.horizontal(|ui| {
        ui.label("orquestrador:");
        ui.text_edit_singleline(&mut agents.url);
        if ui.button("Atualizar").clicked() {
            agents.refresh();
        }
    });
    if !agents.error.is_empty() {
        ui.label(&agents.error);
    }
    if agents.list.is_empty() {
        ui.label("Nenhuma instância observada. Clique Atualizar.");
        return;
    }
    egui::Grid::new("agents_grid").show(ui, |ui| {
        ui.label("instância");
        ui.label("definição");
        ui.label("modelo");
        ui.label("slots");
        ui.label("concluídos");
        ui.label("falhas");
        ui.label("latência ms");
        ui.end_row();
        for agent in &agents.list {
            ui.label(&agent.agent_id);
            let linked = match store.definition_of(&agent.agent_id) {
                Some(definition) => {
                    let name = if definition.name.is_empty() {
                        &definition.id
                    } else {
                        &definition.name
                    };
                    format!("{name} (revisão {})", definition.revision)
                }
                None => String::from("Não confirmado"),
            };
            ui.label(linked);
            ui.label(&agent.model);
            ui.label(format!("{}/{}", agent.slots_busy, agent.slots_total));
            ui.label(agent.completed_total.to_string());
            ui.label(agent.failed_total.to_string());
            ui.label(format!("{:.1}", agent.ema_latency_ms));
            ui.end_row();
        }
    });
}
