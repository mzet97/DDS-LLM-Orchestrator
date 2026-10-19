//! Parte de `views::shared_catalog` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) fn show_cards(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    // ── 4 cards do snapshot ──
    let leader = if shared.url.is_empty() {
        String::from("—")
    } else {
        kit::authority(&shared.url)
    };
    let (items_len, cursor) = shared
        .snapshot
        .as_ref()
        .map(|snapshot| (snapshot.items.len(), snapshot.cursor.0))
        .unwrap_or((0, 0));
    let mut kinds: Vec<(String, usize)> = Vec::new();
    if let Some(snapshot) = &shared.snapshot {
        for item in &snapshot.items {
            let kind = kind_of(&item.id.0);
            match kinds.iter_mut().find(|(name, _)| name == kind) {
                Some((_, count)) => *count += 1,
                None => kinds.push((kind.to_owned(), 1)),
            }
        }
        kinds.sort();
    }
    let kinds_text = if kinds.is_empty() {
        String::from("—")
    } else {
        kinds
            .iter()
            .map(|(name, count)| format!("{name} {count}"))
            .collect::<Vec<_>>()
            .join(" · ")
    };
    let last_write = shared
        .feed
        .iter()
        .rev()
        .find(|entry| matches!(entry.tag, FeedTag::Committed | FeedTag::Tombstone));
    let tomb_sub = format!("{} tombstone(s) nesta sessão", shared.session_tombstones);
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Revisão atual",
            format!("rev.{cursor}"),
            if shared.snapshot.is_some() {
                "snapshot aplicado"
            } else {
                "sem snapshot"
            },
            theme::PRIMARY_FIXED_DIM,
        );
        kit::metric_card(
            &mut cols[1],
            "Total de registros",
            format!("{} itens", items_len),
            &tomb_sub,
            theme::OK,
        );
        kit::metric_card(
            &mut cols[2],
            "Tipos mapeados",
            kinds.len().to_string(),
            &kinds_text,
            theme::ON_SURFACE_VARIANT,
        );
        // O design mostra origem no card (host/WRITER), nunca o texto do
        // feed: repetir `entry.text` aqui duplicava o nó accesskit
        // (multi-match) e divergia do PNG. Writer por evento não trafega
        // no contrato — a origem honesta é a autoridade que aplicou.
        let last_write_via = format!("via {leader}");
        match last_write {
            Some(entry) => kit::metric_card(
                &mut cols[3],
                "Última gravação",
                format!("há {}", ago(entry.ts_ms)),
                &last_write_via,
                theme::WARN,
            ),
            None => kit::metric_card(
                &mut cols[3],
                "Última gravação",
                String::from("—"),
                "nenhuma gravação na sessão",
                theme::STALE,
            ),
        }
    });
    ui.add_space(theme::SPACE_MD);
}
