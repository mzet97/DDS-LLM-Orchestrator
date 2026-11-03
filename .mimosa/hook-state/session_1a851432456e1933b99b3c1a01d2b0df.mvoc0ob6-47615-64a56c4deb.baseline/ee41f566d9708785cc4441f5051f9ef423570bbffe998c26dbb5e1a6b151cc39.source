//! Implementação de `eframe::App` do StudioApp (extraída de main.rs).

use super::*;

mod central;
mod header;
mod header_target;
mod nav;
mod status;
mod target;

impl eframe::App for StudioApp {
    fn ui(&mut self, ui: &mut egui::Ui, _frame: &mut eframe::Frame) {
        self.render_header(ui);
        self.sync_target(ui);
        self.render_status(ui);
        self.render_nav(ui);
        self.render_central(ui);
    }
}
