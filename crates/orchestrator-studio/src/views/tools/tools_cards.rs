//! Parte de `views` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_cards(ui: &mut egui::Ui, dds: &mut DdsState) -> bool {
    let tools = &dds.snapshot.tools;
    // ── 3 cards de nível (vocabulário PRD 3.13) com contagens reais ──
    let level_count = |level: i32| tools.iter().filter(|t| t.security_level == level).count();
    let higher = tools.iter().filter(|t| t.security_level > 2).count();
    // Bloqueios DO N2 (o total da janela já aparece nos filtros; o sub do
    // N2 com o total global mentia quando o DENIED era de outro nível).
    let denied_n2 = tools
        .iter()
        .filter(|t| t.security_level == 2 && t.status == 2)
        .count();
    ui.columns(3, |cols| {
        kit::metric_card(
            &mut cols[0],
            "N0 READ_ONLY",
            level_count(0).to_string(),
            "leitura pura — sem efeito no host",
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "N1 SANDBOX_EXEC",
            level_count(1).to_string(),
            "execução isolada (sandbox)",
            theme::WARN,
        );
        kit::metric_card(
            &mut cols[2],
            "N2 HOST_MUTATION",
            level_count(2).to_string(),
            &format!(
                "mutação no host · {} nível(is) fora da escala{}",
                higher,
                if denied_n2 > 0 {
                    format!(" · {denied_n2} bloqueio(s) DENIED")
                } else {
                    String::new()
                }
            ),
            theme::ERROR,
        );
    });
    ui.add_space(theme::SPACE_MD);

    if tools.is_empty() {
        kit::empty_state(
            ui,
            "Nenhuma tool call na janela. Suba mcp-gateway + policy-engine no \
             domínio e gere uma chamada (ex.: um agente com engine LLM que \
             emita tool_call).",
        );
        return false;
    }

    true
}
