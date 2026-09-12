//! Painel de máquinas e rede (T03): tabela com saúde, confiança,
//! evidência DDS, serviços, origem e observação em colunas distintas.
//! Ações emitem intenções; a tela não implementa scanner.

use eframe::egui;
use orchestrator_studio::machines::MachineLedger;

/// Registro, tabela e detalhes por abas da página.
pub fn show(ui: &mut egui::Ui, ledger: &mut MachineLedger) {
    ui.heading("Máquinas e rede");
    ui.label("Saúde, confiança e possibilidade de administração são campos distintos.");
    ui.horizontal(|ui| {
        ui.label("apelido:");
        ui.text_edit_singleline(&mut ledger.draft_alias);
        ui.label("endpoint:");
        ui.text_edit_singleline(&mut ledger.draft_endpoint);
        if ui.button("Adicionar máquina").clicked() {
            let alias = ledger.draft_alias.clone();
            let endpoint = ledger.draft_endpoint.clone();
            match ledger.add(&alias, &endpoint) {
                Ok(()) => {
                    ledger.draft_alias.clear();
                    ledger.draft_endpoint.clear();
                    ledger.error.clear();
                }
                Err(err) => ledger.error = err,
            }
        }
        if !ledger.error.is_empty() {
            ui.label(&ledger.error);
        }
    });
    if ledger.list().is_empty() {
        ui.label("Nenhuma máquina cadastrada. Adicione pelo apelido e endpoint.");
        return;
    }
    let mut picked: Option<String> = None;
    orchestrator_studio::machines::show_table(ui, ledger, &mut |alias| {
        picked = Some(String::from(alias));
    });
    if let Some(alias) = picked {
        ledger.select(&alias);
    }
    ui.horizontal(|ui| {
        if ui.button("Remover selecionada").clicked() {
            if let Some(current) = ledger.selected().map(|entry| entry.alias.clone()) {
                ledger.remove(&current);
            }
        }
    });
    if let Some(entry) = ledger.selected() {
        ui.separator();
        ui.monospace(format!("{} · {}", entry.alias, entry.endpoint));
        egui::Grid::new("machine_detail").show(ui, |ui| {
            ui.label("conectividade administrativa:");
            ui.label(entry.admin.label());
            ui.end_row();
            ui.label("confiança na identidade:");
            ui.label(entry.trust.label());
            ui.end_row();
            ui.label("evidência DDS:");
            ui.label(entry.dds.label());
            ui.end_row();
            ui.label("origem:");
            ui.label(&entry.origin);
            ui.end_row();
            ui.label("última observação:");
            ui.label(&entry.last_seen);
            ui.end_row();
        });
        ui.group(|ui| {
            ui.strong("Capacidade indisponível");
            ui.label("Reconectar, solicitar adoção e atualizar inventário exigem o serviço de implantação (G-INT-01). Intenções registradas, nada executado.");
        });
    }
}
