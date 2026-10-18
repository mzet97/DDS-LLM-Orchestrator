//! Kit de componentes visuais compartilhados (migração Stitch → egui;
//! reforma visual UX3: hierarquia tipográfica forte, pills com borda,
//! cabeçalho de tabela em faixa, cards com borda de acento).
//! Sem sombra — profundidade por luminância (DESIGN.md).

use crate::theme;
use eframe::egui;

mod kit_cards;
mod kit_text;
mod kit_widgets;

pub use kit_cards::*;
pub use kit_text::*;
pub use kit_widgets::*;

#[cfg(test)]
mod tests;
