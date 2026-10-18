//! Parte de `views::node` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_ops_section(
    ui: &mut egui::Ui,
    state: &mut AppState,
    _node_url: &mut String,
    _node_token: &mut String,
    _discovery: &DiscoveryState,
) {
    // ── Log de operações + inspetor (mockup: filtro, 4 colunas honestas) ──
    let operations: &[studio_node::operations::OpRecord] = state
        .node()
        .map(|node| node.operations.as_slice())
        .unwrap_or(&[]);
    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            show_ops_log(ui, operations);
        });
        cols[1].vertical(|ui| {
            show_inspector(ui, operations);
        });
    });
    ui.add_space(theme::SPACE_LG);
}
