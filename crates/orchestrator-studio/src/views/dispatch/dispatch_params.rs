//! Parte de `views::dispatch` (extraída para manter os arquivos revisáveis).

use super::*;

use crate::workload::DispatchOutcome;

pub(super) fn conn_chip(dispatch: &DispatchState) -> (String, egui::Color32) {
    if dispatch.busy {
        return (String::from("○ DESPACHANDO…"), theme::WARN);
    }
    match &dispatch.last {
        Some(DispatchOutcome::Completed { .. }) => {
            let port = dispatch
                .url
                .rsplit(':')
                .next()
                .filter(|part| !part.is_empty() && part.chars().all(|c| c.is_ascii_digit()));
            let text = match port {
                Some(digits) => format!("● CONECTADO · :{digits}"),
                None => String::from("● CONECTADO"),
            };
            (text, theme::OK)
        }
        Some(DispatchOutcome::Failed { .. }) => (String::from("× FALHA NO BACKEND"), theme::ERROR),
        None if dispatch.result.starts_with("erro de despacho") => {
            (String::from("× ORQUESTRADOR INALCANÇÁVEL"), theme::ERROR)
        }
        None if !dispatch.result.is_empty() => {
            (String::from("○ ÚLTIMO ENVIO COM ERRO"), theme::WARN)
        }
        None => (String::from("○ NUNCA TESTADO"), theme::STALE),
    }
}

/// Coluna esquerda: parâmetros reais do despacho.
pub(super) fn params_column(
    ui: &mut egui::Ui,
    dispatch: &mut DispatchState,
    discovery: &DiscoveryState,
) {
    // section_label consome a linha toda — em fileiras usa-se rótulo plano.
    ui.horizontal(|ui| {
        ui.label(header("▶ PARÂMETROS DE DESPACHO"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(egui::RichText::new("HTTP POST DIRETO").small().weak());
        });
    });
    ui.add_space(theme::SPACE_XS);

    ui.label(caption("ENDPOINT DO ORQUESTRADOR"));
    ui.horizontal(|ui| {
        ui.label(egui::RichText::new("URI:").monospace().small());
        ui.add(
            egui::TextEdit::singleline(&mut dispatch.url)
                .desired_width(f32::INFINITY)
                .hint_text("http://127.0.0.1:8085"),
        );
    });
    ui.label(
        egui::RichText::new("/api/v1/chat/completions/sync (caminho fixo no fio)")
            .monospace()
            .small()
            .weak(),
    );
    ui.add_space(theme::SPACE_XS);

    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(caption("SELETOR DE AGENTE / MODELO"));
            agent_combo(ui, dispatch, discovery);
        });
        ui.separator();
        ui.vertical(|ui| {
            ui.label(caption("MODO DE EXECUÇÃO"));
            ui.label(
                egui::RichText::new("● Síncrono (Wait-for-completion)")
                    .color(theme::PRIMARY_FIXED_DIM),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);

    // Trio numérico real (stop-sequence do mockup não existe no backend).
    ui.horizontal(|ui| {
        ui.vertical(|ui| {
            ui.label(caption("TIMEOUT (MS)"));
            ui.add(
                egui::TextEdit::singleline(&mut dispatch.timeout_ms)
                    .desired_width(90.0)
                    .hint_text("300000"),
            );
        });
        ui.vertical(|ui| {
            ui.label(caption("TEMPERATURE"));
            ui.add(
                egui::TextEdit::singleline(&mut dispatch.temperature)
                    .desired_width(70.0)
                    .hint_text("0.7"),
            );
        });
        ui.vertical(|ui| {
            ui.label(caption("MAX TOKENS"));
            ui.add(
                egui::TextEdit::singleline(&mut dispatch.max_tokens)
                    .desired_width(70.0)
                    .hint_text("256"),
            );
        });
    });
    ui.add_space(theme::SPACE_XS);

    ui.horizontal(|ui| {
        ui.label(caption("PAYLOAD DE PROMPT / INSTRUÇÃO TÉCNICA"));
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new("[CTRL + ENTER PARA ENVIAR]")
                    .monospace()
                    .small()
                    .weak(),
            );
        });
    });
    ui.add(
        egui::TextEdit::multiline(&mut dispatch.prompt)
            .desired_rows(8)
            .desired_width(f32::INFINITY)
            .hint_text("ex.: resuma o estado do nó alvo em 3 bullets"),
    );
    // Ctrl+Enter despacha (o multiline não consome o atalho).
    if ui.input(|state| state.modifiers.ctrl && state.key_pressed(egui::Key::Enter))
        && !dispatch.busy
        && !dispatch.prompt.trim().is_empty()
    {
        dispatch.send();
    }
    ui.add_space(theme::SPACE_XS);

    ui.horizontal(|ui| {
        let send = ui.add_enabled(
            !dispatch.busy && !dispatch.prompt.trim().is_empty(),
            egui::Button::new(
                egui::RichText::new(if dispatch.busy {
                    "○ despachando… (agente real infere)"
                } else {
                    "▶ Despachar Task Síncrona"
                })
                .monospace()
                .color(theme::ON_PRIMARY),
            )
            .fill(theme::PRIMARY_CONTAINER),
        );
        if send.clicked() {
            dispatch.send();
        }
        if ui.button("Limpar").clicked() {
            dispatch.prompt.clear();
        }
    });

    // Erros de validação/rede aparecem SÓ aqui (o card da direita cobre o
    // desfecho estruturado; "falha {id}" pertence ao card, não ao banner).
    if dispatch.result.contains("inválid") || dispatch.result.starts_with("erro de despacho") {
        ui.add_space(theme::SPACE_XS);
        kit::error_banner(ui, &dispatch.result);
    }
}

/// Combo de agentes vivos (DDS): escolher preenche o modelo efetivo; sem
/// agentes, o campo de texto abaixo segue editável (modelo manual).
pub(super) fn agent_combo(
    ui: &mut egui::Ui,
    dispatch: &mut DispatchState,
    discovery: &DiscoveryState,
) {
    let live = &discovery.agents;
    egui::ComboBox::from_id_salt("dispatch_agent_model")
        .selected_text(if live.is_empty() {
            String::from("nenhum agente no domínio — modelo manual")
        } else {
            format!("{} agente(s) no domínio — escolher", live.len())
        })
        .show_ui(ui, |ui| {
            for agent in live {
                ui.selectable_value(
                    &mut dispatch.model,
                    agent.model.clone(),
                    format!("{} ({})", agent.agent_id, agent.model),
                );
            }
        });
    ui.add(
        egui::TextEdit::singleline(&mut dispatch.model)
            .desired_width(220.0)
            .hint_text("modelo efetivo"),
    );
}

pub(super) fn caption(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .small()
        .strong()
        .color(theme::ON_SURFACE_VARIANT)
}

/// Título de seção para fileiras (o `section_label` do kit consome a linha
/// toda e quebra `horizontal`: some badge, corta o bloco direito).
pub(super) fn header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .small()
        .strong()
        .color(theme::OUTLINE)
}
