//! Parte de `views::tools` (extraída para manter os arquivos revisáveis).

use super::*;

/// Título de seção (o `section_label` do kit centraliza — achado 3.12).
pub(super) fn header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .small()
        .strong()
        .color(theme::OUTLINE)
}

/// Seção com hairline, título à esquerda (espelho do PNG).
pub(super) fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(header(text));
    ui.separator();
    ui.add_space(theme::SPACE_SM);
}

/// ID curto para a tabela: até 8 chars mostra tudo, acima disso os
/// ÚLTIMOS 8 (IDs reais `probe-1a…` diferem só no fim — take do início
/// colidia; `…` no meio estourou a célula comprimida e o egui elidiu o
/// sufixo — captura 3.13 + probe kittest). Íntegra no hover.
pub(super) fn short_call_id(call_id: &str) -> String {
    if call_id.chars().count() <= 8 {
        return call_id.to_owned();
    }
    call_id
        .chars()
        .rev()
        .take(8)
        .collect::<String>()
        .chars()
        .rev()
        .collect()
}

/// Inteiro com separador de milhar (legibilidade de durações grandes).
pub(super) fn fmt_int(n: u64) -> String {
    let digits = n.to_string();
    let mut grouped = String::with_capacity(digits.len() + digits.len() / 3);
    for (i, ch) in digits.chars().rev().enumerate() {
        if i > 0 && i % 3 == 0 {
            grouped.push(',');
        }
        grouped.push(ch);
    }
    grouped.chars().rev().collect()
}
