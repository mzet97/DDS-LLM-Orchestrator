//! Painel 3.9 Catálogo Compartilhado (multi-host, OCC): autoridade +
//! sincronização, banner 409 estruturado (base vs vigente), 4 cards, gaveta
//! de publicação com write-lock, busca+filtros, tabela com ações e feed de
//! mutações com relógio e etiquetas.
//!
//! Só dados reais: revisões são POR CHAVE (sem "recém-atualizado" global),
//! tombstones vivem no nó (aqui só os da sessão), writer do conflito não
//! trafega (pílula sem "por ...") e o canal é HTTP follow — nada de SSE.

use crate::catalog_remote::{FeedTag, SharedCatalog};
use crate::kit;
use crate::panel_header::panel_header;
use crate::theme;
use eframe::egui;

/// Kind honesto de um id: o prefixo antes de ':' (ex. `machine:orch-62`).
fn kind_of(id: &str) -> &str {
    id.split(':').next().unwrap_or("sem-prefixo")
}

/// `true` quando o aviso corrente é um conflito OCC (estruturado ou texto).
fn conflict_active(shared: &SharedCatalog) -> bool {
    shared.conflict.is_some() || shared.notice.contains("base obsoleta")
}

/// Linha visível sob kind + busca (busca casa id OU valor).
fn row_visible(id: &str, value: &str, kind: &str, filter: Option<&str>, search: &str) -> bool {
    if filter.is_some_and(|filter| filter != kind) {
        return false;
    }
    let query = search.trim();
    if query.is_empty() {
        return true;
    }
    id.contains(query) || value.contains(query)
}

pub fn show(ui: &mut egui::Ui, shared: &mut SharedCatalog) {
    panel_header(
        ui,
        "CAT-09 :: STATE_GOV",
        "3.9 Catálogo Compartilhado",
        "Catálogo Distribuído Multi-Host · Autoridade Primária no Nó Selecionado",
    );
    // Drena o worker de HTTP (thread + mpsc — REQ/T-820-19).
    shared.poll();

    show_authority(ui, shared);
    show_banner(ui, shared);
    show_cards(ui, shared);
    show_drawer(ui, shared);
    show_table(ui, shared);
    show_feed(ui, shared);
}

mod shared_authority;
mod shared_banner;
mod shared_cards;
mod shared_drawer;
mod shared_feed;
mod shared_helpers;
mod shared_table;

use shared_authority::show_authority;
use shared_banner::show_banner;
use shared_cards::show_cards;
use shared_drawer::show_drawer;
use shared_feed::show_feed;
use shared_helpers::{ago, caption, header};
use shared_table::show_table;

#[cfg(test)]
mod tests;
