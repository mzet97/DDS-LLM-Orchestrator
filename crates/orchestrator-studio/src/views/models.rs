//! Painel 3.7 Modelos GGUF (inventário SHA-256): diretório + manifesto,
//! banner de progresso do worker real, 4 cards de resumo, filtros por status,
//! tabela calculado × manifesto com ações e rodapé do motor de checksum.
//!
//! Só dados reais: engine `sha2` (nada de BLAKE3/AVX-512/OpenSSL), hostname e
//! filesystem via libc, sem tópico DDS nem lease de verificador (o painel é
//! local — esses itens do mockup não existem e ficam FORA).

use crate::kit;
use crate::models::{ManifestStatus, ModelsState};
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, state: &mut ModelsState) {
    panel_header(
        ui,
        "SEC 3.7",
        "3.7 Modelos GGUF",
        "Inventário de tensores locais, validação e verificação de integridade criptográfica contra manifesto imutável.",
    );
    ui.horizontal(|ui| {
        kit::badge(ui, "MANIFESTO SHA-256", theme::PRIMARY_FIXED_DIM);
        kit::badge(
            ui,
            &format!("STORAGE-HOST: {}", hostname()),
            theme::ON_SURFACE_VARIANT,
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            if ui.button("Exportar Relatório").clicked() {
                state.export_report();
            }
            let verify = ui.add_enabled(
                !state.list.is_empty() && !state.is_busy(),
                egui::Button::new("↻ Verificar Tudo"),
            );
            if verify.clicked() {
                state.verify_all();
            }
        });
    });
    ui.add_space(theme::SPACE_SM);

    state.poll();
    if state.is_busy() {
        ui.ctx().request_repaint();
    }

    show_directory(ui, state);
    show_progress(ui, state);
    show_cards(ui, state);
    show_artifacts(ui, state);
    show_table(ui, state);
}

mod models_artifacts;
mod models_cards;
mod models_directory;
mod models_helpers;
mod models_progress;
mod models_table;

use models_artifacts::show_artifacts;
use models_cards::show_cards;
use models_directory::show_directory;
use models_helpers::{caption, fmt_gb2, fmt_hhmmss, fs_info, fs_total_bytes, hostname, short_sha};
use models_progress::show_progress;
use models_table::show_table;

#[cfg(test)]
mod tests;
