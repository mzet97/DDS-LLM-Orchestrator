//! Parte de `views::inference` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_composer(
    ui: &mut egui::Ui,
    infer: &mut InferenceState,
    _discovery: &DiscoveryState,
) {
    // ── Composer (mockup) ──
    let composer = ui.add(
        egui::TextEdit::multiline(&mut infer.prompt)
            .desired_rows(3)
            .desired_width(ui.available_width())
            .hint_text(
                "Digite o comando ou questão técnica para o cluster de inferência… \
                 (Ex.: Como renegociar QoS de Durability via IDL sem reiniciar nós?)",
            ),
    );
    ui.add_space(theme::SPACE_XS);
    let chars = infer.prompt.chars().count();
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(format!(
                "{chars} caracteres · ~{} tokens (estimativa ×/4)",
                chars / 4
            ))
            .monospace()
            .small()
            .color(theme::OUTLINE),
        );
        ui.label(
            egui::RichText::new("Atalho: Ctrl + Enter para enviar")
                .small()
                .color(theme::OUTLINE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            let can_send = !infer.busy && !infer.prompt.trim().is_empty();
            let send = ui.add_enabled(
                can_send,
                egui::Button::new(
                    egui::RichText::new(if infer.busy {
                        "gerando…"
                    } else {
                        "▶ ENVIAR PROMPT"
                    })
                    .monospace()
                    .color(theme::ON_PRIMARY),
                )
                .fill(theme::PRIMARY_CONTAINER),
            );
            let ctrl_enter = composer.has_focus()
                && ui.input(|i| {
                    (i.modifiers.ctrl || i.modifiers.command) && i.key_pressed(egui::Key::Enter)
                });
            if (send.clicked() || ctrl_enter) && can_send {
                infer.send();
                crate::studio_log::info("inferência: prompt enviado ao servidor");
            }
            if ui
                .add_enabled(infer.busy, egui::Button::new("Parar Geração"))
                .clicked()
            {
                infer.cancel();
            }
        });
    });
    if !infer.reply.is_empty()
        && (infer.reply.starts_with("transcript") || infer.reply.starts_with("falha"))
    {
        ui.label(
            egui::RichText::new(&infer.reply)
                .monospace()
                .small()
                .color(theme::ON_SURFACE_VARIANT),
        );
    }
}
