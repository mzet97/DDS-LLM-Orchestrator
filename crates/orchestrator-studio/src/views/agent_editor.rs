//! Assistente de agente (T05): delega ao render da lib sobre o estado.

use eframe::egui;
use orchestrator_studio::agent_editor::{self, AgentEditor};

/// Passo a passo de criação; edição usa a mesma estrutura em página.
pub fn show(ui: &mut egui::Ui, editor: &mut AgentEditor) {
    agent_editor::show(ui, editor);
}
