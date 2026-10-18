//! Parte de `views::workflow` (extraída para manter os arquivos revisáveis).

/// Tabela de auditoria dos frames entregues (mesma fonte dos cards).
/// FORA: AGENTE EXECUTOR (sem executor no `TaskResult`) e TIMESTAMP RTPS
/// (sem carimbo por estágio no fio).
#[cfg(feature = "dds")]
use super::*;

pub(super) fn audit_table(ui: &mut egui::Ui, state: &mut WorkflowState) {
    if state.stages.is_empty() {
        return;
    }
    ui.horizontal(|ui| {
        ui.label(header(&format!(
            "AUDITORIA DE FRAMES DDS NO DOMÍNIO {}",
            state.config.domain
        )));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(format!("{} / 3 DELIVERED", state.stages.len()))
                    .monospace()
                    .small()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);
    kit::table("workflow_stages").show(ui, |ui| {
        kit::grid_header(
            ui,
            &["Estágio", "Task ID", "Latência", "Buffer (prévia 96 chars)"],
        );
        for stage in &state.stages {
            ui.label(egui::RichText::new(stage_display(&stage.stage)).strong());
            ui.label(
                egui::RichText::new(stage.task_id.chars().take(8).collect::<String>())
                    .monospace()
                    .color(theme::ON_SURFACE_VARIANT),
            )
            .on_hover_text(&stage.task_id);
            kit::num_cell(ui, &format!("{} ms", fmt_int(stage.latency_ms)));
            ui.label(egui::RichText::new(&stage.preview).small().weak());
            ui.end_row();
        }
    });
    if let Some(total_ms) = state.total_ms {
        if state.error.is_none() {
            ui.label(
                egui::RichText::new(format!(
                    "total: {} ms (3 estágios encadeados)",
                    fmt_int(total_ms)
                ))
                .monospace()
                .small()
                .color(theme::OK),
            );
        }
    }
    ui.add_space(theme::SPACE_MD);
}

/// Título de seção para fileiras (o `section_label` do kit consome a linha
/// toda e quebra `horizontal` — ver 3.6).
#[cfg(feature = "dds")]
pub(super) fn header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .small()
        .strong()
        .color(theme::OUTLINE)
}

/// Seção com hairline (os títulos saíram centralizados com o
/// `section_label` do kit na captura 3.12; aqui à esquerda, espelho do PNG).
#[cfg(feature = "dds")]
pub(super) fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(header(text));
    ui.separator();
    ui.add_space(theme::SPACE_SM);
}

/// Tratamento de exceções: o erro REAL do estágio que falhou. FORA:
/// "SIMULAÇÃO DE FALHA" (não existe simulação), caixas nó/agente/postura e
/// trace RTPS (sem fonte no fio).
#[cfg(feature = "dds")]
pub(super) fn exceptions(ui: &mut egui::Ui, state: &mut WorkflowState) {
    section(ui, "TRATAMENTO DE EXCEÇÕES & TIMEOUT POR ESTÁGIO (§3.12)");
    if let Some(error) = &state.error {
        kit::error_banner(
            ui,
            &format!(
                "{error} — o pipeline para no primeiro estágio que falha (sem retry silencioso)"
            ),
        );
    } else if state.busy {
        ui.label(
            egui::RichText::new("executando — falhas aparecem aqui com o motivo real")
                .small()
                .weak(),
        );
    } else {
        ui.label(
            egui::RichText::new("nenhuma exceção na última execução")
                .small()
                .color(theme::OK),
        );
    }
}
