//! Parte de `views::models` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_directory(ui: &mut egui::Ui, state: &mut ModelsState) {
    // ── Diretório + manifesto (botões no topo: cliques kittest) ──
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(caption(&format!(
                "DIRETÓRIO DE MODELOS · {}",
                fs_info(&state.dir).unwrap_or_else(|| String::from("—"))
            )));
            let mut dir = state.dir.display().to_string();
            if ui
                .add(
                    egui::TextEdit::singleline(&mut dir)
                        .desired_width(300.0)
                        .hint_text("STUDIO_MODELS_DIR ou $HOME/tese/models"),
                )
                .changed()
            {
                state.dir = dir.into();
            }
        });
        ui.vertical(|ui| {
            ui.label(caption("CAMINHO DO MANIFESTO · SHA-256 IMUTÁVEL"));
            ui.add(
                egui::TextEdit::singleline(&mut state.manifest_path)
                    .desired_width(300.0)
                    .hint_text("benchmarks/orchestration/locks/models-manifest.json"),
            );
        });
        ui.vertical(|ui| {
            ui.label(caption("AÇÕES"));
            ui.horizontal(|ui| {
                if kit::primary_button(ui, "Inventariar").clicked() && !state.is_busy() {
                    state.refresh();
                }
                if ui.button("Carregar manifesto").clicked() {
                    state.load_manifest();
                }
            });
        });
    });
    if !state.notice.is_empty() {
        ui.label(egui::RichText::new(&state.notice).small().color(theme::OK));
    }
    if !state.error.is_empty() {
        kit::error_banner(ui, &state.error);
    }
    if state.dir.as_os_str().is_empty() {
        kit::empty_state(
            ui,
            "Nenhum diretório padrão: defina STUDIO_MODELS_DIR (ou edite o campo acima).",
        );
    }
}
