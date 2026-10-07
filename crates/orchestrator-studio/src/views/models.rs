//! Painel 3.7 Modelos GGUF (inventário SHA-256): filtros por status, 4 cards
//! de resumo, colunas separadas de checksum calculado × manifesto e
//! exportação de relatório local. P4 local; nada é criado nem deletado.

use crate::kit;
use crate::models::{ManifestStatus, ModelsState};
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, state: &mut ModelsState) {
    panel_header(
        ui,
        "SEC 3.7 · MODELOS GGUF — INVENTÁRIO SHA-256",
        "Modelos GGUF",
        "Inventário de artefatos no disco + verificação SHA-256 contra \
         manifesto congelado (hash async com cancelamento) · leitura apenas",
    );
    state.poll();

    // ── Toolbar: diretório + ações (âncoras dos kittest preservadas) ──
    ui.horizontal(|ui| {
        let mut dir = state.dir.display().to_string();
        ui.label("Diretório de modelos:");
        if ui
            .add(
                egui::TextEdit::singleline(&mut dir)
                    .desired_width(320.0)
                    .hint_text("STUDIO_MODELS_DIR ou $HOME/tese/models"),
            )
            .changed()
        {
            state.dir = dir.into();
        }
        if ui.button("Inventariar").clicked() && !state.is_busy() {
            state.refresh();
        }
        if state.is_busy() && ui.button("Cancelar hash").clicked() {
            state.cancel();
        }
        if ui.button("Exportar relatório").clicked() {
            state.export_report();
        }
    });
    ui.horizontal(|ui| {
        ui.label("Manifesto (SHA-256 congelado):");
        ui.add(
            egui::TextEdit::singleline(&mut state.manifest_path)
                .desired_width(380.0)
                .hint_text("benchmarks/orchestration/locks/models-manifest.json"),
        );
        if ui.button("Carregar manifesto").clicked() {
            state.load_manifest();
        }
    });
    if state.is_busy() {
        ui.ctx().request_repaint();
    }
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

    // ── Progresso rico (PRD 3.7): arquivo corrente · n/total · % · MB/s
    //    medidos · ETA · Cancelar — tudo deriva do worker de hash real ──
    if let Some(progress) = &state.hashing {
        let rate_mb_s = progress.rate_bps as f64 / (1024.0 * 1024.0);
        let mut detail = format!(
            "{}/{} verificados · corrente: {}",
            progress.done, progress.total, progress.current
        );
        if progress.current_total > 0 {
            detail.push_str(&format!(" · {:.1} MB/s", rate_mb_s));
            if let Some(eta) = progress.current_eta_secs() {
                detail.push_str(&format!(" · ETA ~{eta}s"));
            }
        }
        // Barra dupla: corrente (medida) na frente, lote (n/total) atrás.
        ui.horizontal(|ui| {
            ui.label(
                egui::RichText::new("CALCULANDO SHA-256:")
                    .monospace()
                    .small()
                    .color(theme::ON_SURFACE_VARIANT),
            );
            ui.add(
                egui::ProgressBar::new(progress.done as f32 / progress.total.max(1) as f32)
                    .desired_width(220.0),
            );
            ui.add(
                egui::ProgressBar::new(progress.current_fraction())
                    .desired_width(120.0)
                    .fill(theme::PRIMARY_CONTAINER)
                    .text(""),
            );
            ui.label(egui::RichText::new(detail).small().weak());
        });
    }

    // Estado do manifesto (âncora literal: "manifesto carregado: N registro(s)").
    match (&state.manifest, state.manifest_error.is_empty()) {
        (Some(manifest), true) => {
            ui.label(
                egui::RichText::new(format!(
                    "manifesto carregado: {} registro(s)",
                    manifest.len()
                ))
                .monospace()
                .small()
                .color(theme::OK),
            );
        }
        (None, true) => {
            ui.label(
                egui::RichText::new("Sem manifesto carregado: os SHAs ficam sem cruzamento.")
                    .monospace()
                    .small()
                    .color(theme::STALE),
            );
        }
        _ => {
            kit::error_banner(ui, &state.manifest_error);
        }
    }
    ui.add_space(theme::SPACE_MD);

    // ── 4 cards de resumo (mockup 3.7) ──
    let total_bytes: u64 = state.list.iter().map(|item| item.size_bytes).sum();
    let ok_count = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Ok)
        .count();
    let deviant_count = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Desviado)
        .count();
    let pending_count = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Pendente)
        .count();
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Volume mapeado",
            format!("{:.1} GiB", total_bytes as f64 / 1_073_741_824.0),
            &format!("{} arquivo(s) .gguf", state.list.len()),
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Verificados OK",
            ok_count.to_string(),
            "SHA confere com o manifesto",
            theme::OK,
        );
        kit::metric_card(
            &mut cols[2],
            "Divergências (hash)",
            deviant_count.to_string(),
            if deviant_count > 0 {
                "DESVIADO — não é o congelado"
            } else {
                "nenhuma divergência"
            },
            if deviant_count > 0 {
                theme::ERROR
            } else {
                theme::OK
            },
        );
        kit::metric_card(
            &mut cols[3],
            "Pipeline de hash",
            if state.is_busy() {
                "1".to_owned()
            } else {
                "0".to_owned()
            },
            if state.is_busy() {
                "verificação em andamento"
            } else {
                "ocioso"
            },
            if state.is_busy() {
                theme::WARN
            } else {
                theme::STALE
            },
        );
    });
    ui.add_space(theme::SPACE_MD);

    if state.list.is_empty() && !state.is_busy() {
        kit::empty_state(
            ui,
            "Nenhum .gguf listado. Ajuste o diretório e clique em Inventariar.",
        );
        return;
    }

    // ── Filtros por status (mockup 3.7) ──
    ui.horizontal(|ui| {
        let filters: [(u8, String); 3] = [
            (0, format!("Todos ({})", state.list.len())),
            (1, format!("Divergentes ({deviant_count})")),
            (2, format!("Pendentes ({pending_count})")),
        ];
        for (id, label) in filters {
            if ui
                .selectable_label(
                    state.filter == id,
                    egui::RichText::new(label).monospace().small(),
                )
                .clicked()
            {
                state.filter = id;
            }
        }
    });
    ui.add_space(theme::SPACE_SM);

    // ── Tabela: checksum calculado × manifesto em colunas separadas ──
    egui::Grid::new("models-artifacts")
        .striped(true)
        .show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "Arquivo",
                    "Tamanho",
                    "Checksum SHA-256 calculado",
                    "Checksum do manifesto",
                    "Status",
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
                let calculated = if artifact.sha256_hex.is_empty() {
                    egui::RichText::new("calculando…")
                        .small()
                        .color(theme::WARN)
                } else {
                    egui::RichText::new(&artifact.sha256_hex)
                        .monospace()
                        .small()
                };
                ui.label(calculated);
                let expected = state
                    .manifest
                    .as_ref()
                    .and_then(|manifest| manifest.expected(&artifact.file_name));
                match expected {
                    Some(sha) => {
                        let short: String = sha.chars().take(16).collect();
                        ui.label(
                            egui::RichText::new(format!("{short}…"))
                                .monospace()
                                .small()
                                .color(theme::ON_SURFACE_VARIANT),
                        );
                    }
                    None => {
                        ui.label(
                            egui::RichText::new("— sem registro")
                                .small()
                                .color(theme::STALE),
                        );
                    }
                }
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
                ui.end_row();
            }
        });
    let hashed = state
        .list
        .iter()
        .filter(|item| !item.sha256_hex.is_empty())
        .count();
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
