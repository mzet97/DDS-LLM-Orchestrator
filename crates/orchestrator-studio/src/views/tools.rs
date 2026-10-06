//! Painel "Ferramentas": tool calls ao vivo do mesh (T-890-06, G-18..22).
//!
//! Usa o MESMO `DdsState` da Topologia (uma observação por clique alimenta
//! os dois painéis). Mostra a governança de cada chamada: requester, nível
//! de segurança pedido (decidido pelo policy-engine), status e prévia do
//! resultado escrito pelo `mcp-gateway` na MESMA instância `ToolCall.Request`.

use crate::dds_observe::DdsState;
use eframe::egui;

pub fn show(ui: &mut egui::Ui, dds: &mut DdsState) {
    dds.poll();
    ui.heading("Ferramentas (mesh ao vivo)");
    ui.label(
        "Chamadas observadas no tópico ToolCall.Request: o gateway reivindica, \
         o policy-engine decide pelo nível e o resultado volta na mesma \
         instância (at-least-once, sem tópico de resposta).",
    );
    ui.separator();
    ui.horizontal(|ui| {
        ui.label("domínio:");
        ui.add(egui::DragValue::new(&mut dds.domain));
        ui.label("janela (s):");
        ui.add(egui::DragValue::new(&mut dds.window_secs).range(1..=30));
        ui.add_enabled_ui(!dds.busy, |ui| {
            if ui.button("Observar").clicked() {
                dds.refresh();
            }
        });
    });
    if !dds.error.is_empty() {
        ui.label(&dds.error);
    }
    if dds.snapshot.tools.is_empty() {
        ui.label(
            "Nenhuma tool call na janela. Suba mcp-gateway + policy-engine no \
             domínio e gere uma chamada (ex.: um agente com engine LLM que \
             emita tool_call).",
        );
        return;
    }
    egui::Grid::new("tools_governance_grid")
        .striped(true)
        .show(ui, |ui| {
            ui.strong("call_id");
            ui.strong("ferramenta");
            ui.strong("requester");
            ui.strong("nível");
            ui.strong("status");
            ui.strong("resultado (prévia)");
            ui.end_row();
            for tool in &dds.snapshot.tools {
                ui.monospace(tool.call_id.chars().take(8).collect::<String>());
                ui.label(&tool.tool_name);
                ui.label(&tool.requester_id);
                ui.label(tool.security_level.to_string());
                ui.label(status_label(tool.status).to_owned());
                ui.label(&tool.result_preview);
                ui.end_row();
            }
        });
}

/// Rótulo do status canônico do contrato (`orch-common::ToolCallStatus`:
/// PENDING=0, ALLOWED=1, DENIED=2, EXECUTING=3, COMPLETED=4, FAILED=5).
fn status_label(status: i32) -> &'static str {
    match status {
        0 => "PENDING",
        1 => "ALLOWED",
        2 => "DENIED",
        3 => "EXECUTING",
        4 => "COMPLETED",
        5 => "FAILED",
        _ => "desconhecido",
    }
}
