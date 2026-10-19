//! Parte de `views::models` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_cards(ui: &mut egui::Ui, state: &mut ModelsState) {
    // ── 4 cards de resumo (mockup 3.7) ──
    let total_bytes: u64 = state.list.iter().map(|item| item.size_bytes).sum();
    let ok_count = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Ok)
        .count();
    let deviants: Vec<&str> = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::Desviado)
        .map(|item| item.file_name.as_str())
        .collect();
    let unregistered_count = state
        .list
        .iter()
        .filter(|item| item.manifest_status == ManifestStatus::SemRegistro)
        .count();
    // Subtextos vinculados antes (temporário em `&format!` não sobrevive).
    // Valor do volume CURTO: texto longo quebra em 3 linhas no HERO 32px
    // (provado na captura) — o total do fs vai no subtexto.
    let fs_total = fs_total_bytes(&state.dir);
    let volume_value = format!("{:.2} GB", total_bytes as f64 / 1_000_000_000.0);
    let volume_sub = match fs_total {
        Some(total) => format!(
            "{} arquivo(s) .gguf · {:.1} GB no volume",
            state.list.len(),
            total as f64 / 1_000_000_000.0
        ),
        None => format!("{} arquivo(s) .gguf", state.list.len()),
    };
    let ok_sub = if state.list.is_empty() {
        String::from("nenhum arquivo")
    } else {
        format!(
            "{:.1}% da coleção conferida",
            ok_count as f64 * 100.0 / state.list.len() as f64
        )
    };
    let dev_value = if deviants.is_empty() {
        String::from("0")
    } else {
        format!("{} DESVIADO", deviants.len())
    };
    let dev_sub = if deviants.is_empty() {
        String::from("nenhuma divergência")
    } else {
        format!("{} · quarentena recomendada", deviants.join(", "))
    };
    let pipe_sub = if state.is_busy() {
        format!("EM VERIFICAÇÃO · {unregistered_count} sem manifesto · 1 thread")
    } else {
        String::from("ocioso")
    };
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Volume mapeado",
            volume_value,
            &volume_sub,
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Verificados OK",
            ok_count.to_string(),
            &ok_sub,
            theme::OK,
        );
        kit::metric_card(
            &mut cols[2],
            "Divergências (hash)",
            dev_value,
            &dev_sub,
            if deviants.is_empty() {
                theme::OK
            } else {
                theme::ERROR
            },
        );
        kit::metric_card(
            &mut cols[3],
            "Pipeline de hash",
            if state.is_busy() {
                String::from("1")
            } else {
                String::from("0")
            },
            &pipe_sub,
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
    }
}
