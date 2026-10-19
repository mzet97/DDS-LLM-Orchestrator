//! Parte de `views::models` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_table(ui: &mut egui::Ui, state: &mut ModelsState) {
    let total_bytes: u64 = state.list.iter().map(|item| item.size_bytes).sum();
    // ── Tabela: checksum calculado × manifesto em colunas separadas ──
    kit::table("models-artifacts").show(ui, |ui| {
        kit::grid_header(
            ui,
            &[
                "Nome do Arquivo",
                "Tamanho",
                "Checksum SHA-256 Calculado",
                "Checksum do Manifesto",
                "Status",
                "Ações",
            ],
        );
        for artifact in &state.list {
            let visible = match state.filter {
                1 => artifact.manifest_status == ManifestStatus::Desviado,
                2 => artifact.manifest_status == ManifestStatus::Pendente,
                _ => true,
            };
            if !visible {
                continue;
            }
            kit::mono_cell(ui, &artifact.file_name);
            kit::num_cell(
                ui,
                &format!("{:.1} MiB", artifact.size_bytes as f64 / 1_048_576.0),
            );
            if artifact.sha256_hex.is_empty() {
                match &state.hashing {
                    Some(progress) if progress.current == artifact.file_name => {
                        ui.label(
                            egui::RichText::new(format!(
                                "[Calculando… {:.0}%]",
                                progress.current_fraction() * 100.0
                            ))
                            .small()
                            .color(theme::WARN),
                        );
                    }
                    _ => {
                        ui.label(egui::RichText::new("pendente").small().color(theme::STALE));
                    }
                }
            } else {
                ui.label(
                    egui::RichText::new(short_sha(&artifact.sha256_hex))
                        .monospace()
                        .small(),
                )
                .on_hover_text(&artifact.sha256_hex);
            }
            let expected = state
                .manifest
                .as_ref()
                .and_then(|manifest| manifest.expected(&artifact.file_name));
            match expected {
                Some(sha) => {
                    ui.label(
                        egui::RichText::new(short_sha(sha))
                            .monospace()
                            .small()
                            .color(theme::ON_SURFACE_VARIANT),
                    )
                    .on_hover_text(sha);
                }
                None => {
                    ui.label(
                        egui::RichText::new("— sem registro")
                            .small()
                            .color(theme::STALE),
                    );
                }
            }
            // Status em TEXTO com rótulo EXATO (âncora kittest "OK": sem
            // prefixo ●, sem badge — badge some no accesskit).
            let status_color = match artifact.manifest_status {
                ManifestStatus::Ok => theme::OK,
                ManifestStatus::Desviado => theme::ERROR,
                ManifestStatus::Pendente | ManifestStatus::SemRegistro => theme::STALE,
            };
            ui.label(
                egui::RichText::new(artifact.manifest_status.label())
                    .monospace()
                    .color(status_color),
            );
            let copy = ui.add_enabled(
                !artifact.sha256_hex.is_empty(),
                egui::Button::new("Copiar SHA"),
            );
            if copy.clicked() {
                ui.ctx().copy_text(artifact.sha256_hex.clone());
            }
            ui.end_row();
        }
    });
    let hashed = state
        .list
        .iter()
        .filter(|item| !item.sha256_hex.is_empty())
        .count();
    ui.label(
        egui::RichText::new(
            "Checksum Engine: sha2 software (RustCrypto) · blocos de 64 KiB · 1 thread",
        )
        .monospace()
        .small()
        .color(theme::ON_SURFACE_VARIANT),
    );
    ui.label(
        egui::RichText::new(format!(
            "{} arquivo(s), {} com SHA-256, total {:.1} GiB. Leitura apenas: nada é deletado.",
            state.list.len(),
            hashed,
            total_bytes as f64 / 1_073_741_824.0
        ))
        .small()
        .weak(),
    );
}
