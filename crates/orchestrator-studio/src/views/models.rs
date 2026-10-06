//! Painel de modelos: inventário assíncrono (P4 local).

use crate::models::{ManifestStatus, ModelsState};
use eframe::egui;

pub fn show(ui: &mut egui::Ui, state: &mut ModelsState) {
    state.poll();
    ui.heading("Modelos (arquivos GGUF em disco)");
    ui.horizontal(|ui| {
        let mut dir = state.dir.display().to_string();
        ui.label("Diretório:");
        if ui.text_edit_singleline(&mut dir).changed() {
            state.dir = dir.into();
        }
        if ui.button("Inventariar").clicked() && !state.is_busy() {
            state.refresh();
        }
        if state.is_busy() && ui.button("Cancelar hash").clicked() {
            state.cancel();
        }
    });
    if state.is_busy() {
        ui.ctx().request_repaint();
    }
    if let Some(progress) = &state.hashing {
        ui.add(
            egui::ProgressBar::new(progress.done as f32 / progress.total.max(1) as f32)
                .text(format!(
                    "hash SHA-256 {}/{} — {}",
                    progress.done, progress.total, progress.current
                ))
                .show_percentage(),
        );
    }
    ui.label("P4 local; nada é criado.");
    if state.dir.as_os_str().is_empty() {
        ui.label("Nenhum diretório padrão: defina STUDIO_MODELS_DIR (ou edite o campo acima).");
    }
    if !state.error.is_empty() {
        ui.label(&state.error);
    }

    // T-890-08 (G-09..11): cruzamento GGUF × manifesto congelado.
    ui.separator();
    ui.heading("Manifesto congelado (GGUF × SHA-256)");
    ui.horizontal(|ui| {
        ui.label("manifesto:");
        ui.add(
            egui::TextEdit::singleline(&mut state.manifest_path)
                .desired_width(420.0)
                .hint_text("benchmarks/orchestration/locks/models-manifest.json"),
        );
        if ui.button("Carregar manifesto").clicked() {
            state.load_manifest();
        }
    });
    match (&state.manifest, state.manifest_error.is_empty()) {
        (Some(manifest), true) => {
            ui.label(format!(
                "manifesto carregado: {} registro(s)",
                manifest.len()
            ));
        }
        (None, true) => {
            ui.label("Sem manifesto carregado: os SHAs ficam sem cruzamento.");
        }
        _ => {
            ui.colored_label(egui::Color32::RED, &state.manifest_error);
        }
    }
    if state.list.is_empty() && !state.is_busy() {
        ui.label("Nenhum .gguf listado. Ajuste o diretório e clique em Inventariar.");
        return;
    }
    egui::Grid::new("models-artifacts")
        .striped(true)
        .show(ui, |ui| {
            ui.label("Arquivo");
            ui.label("Tamanho");
            ui.label("SHA-256 (duplo clique seleciona)");
            ui.label("manifesto");
            ui.end_row();
            for artifact in &state.list {
                ui.label(&artifact.file_name);
                ui.label(format!(
                    "{:.1} MiB",
                    artifact.size_bytes as f64 / 1_048_576.0
                ));
                ui.label(if artifact.sha256_hex.is_empty() {
                    "calculando…"
                } else {
                    &artifact.sha256_hex
                });
                let status_color = match artifact.manifest_status {
                    ManifestStatus::Ok => egui::Color32::from_rgb(80, 200, 120),
                    ManifestStatus::Desviado => egui::Color32::RED,
                    ManifestStatus::Pendente | ManifestStatus::SemRegistro => {
                        ui.visuals().weak_text_color()
                    }
                };
                ui.label(egui::RichText::new(artifact.manifest_status.label()).color(status_color));
                ui.end_row();
            }
        });
    let hashed = state
        .list
        .iter()
        .filter(|item| !item.sha256_hex.is_empty())
        .count();
    ui.label(format!(
        "{} arquivo(s), {} com SHA-256, total {:.1} GiB. Leitura apenas: nada é deletado.",
        state.list.len(),
        hashed,
        state.list.iter().map(|item| item.size_bytes).sum::<u64>() as f64 / 1_073_741_824.0
    ));
}
