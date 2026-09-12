//! Catálogo de workflows (T10): sequência, fork-join e serial com
//! dependências explícitas. Cada envio registra UMA intenção.

use eframe::egui;
use orchestrator_studio::workflows::{WorkflowPanel, WorkflowRequest, WorkloadKind};

/// Catálogo + formulário de solicitação + intenções registradas.
pub fn show(ui: &mut egui::Ui, panel: &mut WorkflowPanel) {
    ui.heading("Workflows");
    ui.label("Dependências fiéis ao contrato; arrastar (se houver) só move o desenho.");
    for kind in [
        WorkloadKind::Sequence,
        WorkloadKind::ForkJoin,
        WorkloadKind::Serial,
    ] {
        ui.group(|ui| {
            ui.strong(kind.label());
            ui.label(kind.dependencies());
            if ui.button("Preparar solicitação").clicked() {
                panel.requests.push(WorkflowRequest {
                    workflow_id: format!("wf-{}", panel.requests.len() + 1),
                    kind,
                    profile: String::from("Balanced"),
                    destination: String::new(),
                });
            }
        });
    }
    if panel.requests.is_empty() {
        return;
    }
    ui.separator();
    ui.strong("Solicitações preparadas (intenções, sem execução na GUI):");
    let mut submit: Option<WorkflowRequest> = None;
    for request in &mut panel.requests {
        ui.horizontal(|ui| {
            ui.monospace(format!(
                "{} · {}",
                request.workflow_id,
                request.kind.label()
            ));
            ui.label("destino:");
            ui.text_edit_singleline(&mut request.destination);
            if ui.button("Solicitar execução").clicked() {
                submit = Some(request.clone());
            }
        });
    }
    if let Some(request) = submit {
        panel.submit(request);
    }
    ui.label(format!(
        "{} intenção(ões) registrada(s).",
        panel.intents.len()
    ));
}
