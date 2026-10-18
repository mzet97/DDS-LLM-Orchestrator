//! Painel 3.14 Logs da GUI (FIFO Ring Buffer 500): faixa de stream com
//! taxa e retenção reais, 4 KPIs (ocupação · absorção com densidade ·
//! severidade · sink stderr), filtros por nível com contagem, busca,
//! auto-scroll, espelho stderr, pausa com lista congelada, limpeza,
//! exportação `.log`, tabela invertida (novo primeiro) com offset relativo
//! ao mais novo e inspetor forense (SLOT · THREAD · FONTE · mensagem ·
//! contexto · stack em ERRO · COPIAR DUMP).

use crate::kit;
use crate::panel_header::panel_header;
use crate::studio_log::{self, LogEntry, LogsPanel};
use crate::theme;
use eframe::egui;

/// Níveis fixos do buffer (mesma ordem do enum de push).
const LEVELS: [&str; 3] = ["INFO", "WARN", "ERRO"];

fn level_color(level: &str) -> egui::Color32 {
    match level {
        "ERRO" => theme::ERROR,
        "WARN" => theme::WARN,
        _ => theme::PRIMARY_FIXED_DIM,
    }
}

pub fn show(ui: &mut egui::Ui, panel: &mut LogsPanel) {
    panel_header(
        ui,
        "SEC 3.14 · LOGS DA GUI — FIFO RING BUFFER 500",
        "Logs da GUI",
        "Buffer circular de eventos internos do Studio · espelhado no stderr \
         · FIFO: antigos descartados",
    );
    let live = studio_log::entries();
    // Taxa honesta: eventos ÷ span do buffer (soma dos offsets). Span zero
    // (tudo no mesmo ms) não rende taxa — "—", nunca chute.
    let span_ms: u64 = live.iter().map(|entry| entry.offset_ms).sum();
    let rate = if span_ms > 0 && !live.is_empty() {
        Some(live.len() as f64 / (span_ms as f64 / 1000.0))
    } else {
        None
    };
    let retention = match (live.first(), live.last()) {
        (Some(first), Some(last)) => format!("{} → {}", first.timestamp, last.timestamp),
        _ => String::from("—"),
    };
    let evicted = studio_log::evicted();
    show_stream(ui, panel, &live, rate, &retention, evicted);
    let (toggle_pause, do_clear) = show_toolbar(ui, panel, &live);
    show_table(ui, panel, &live, toggle_pause, do_clear);
    show_inspector(ui, panel, &live);
}

mod logs_helpers;
mod logs_inspector;
mod logs_stream;
mod logs_table;
mod logs_toolbar;

use logs_helpers::{absorption_card, dump, fmt_offset, header, rel_of, rel_offsets, section};
use logs_inspector::show_inspector;
use logs_stream::show_stream;
use logs_table::show_table;
use logs_toolbar::show_toolbar;

#[cfg(test)]
mod tests;
