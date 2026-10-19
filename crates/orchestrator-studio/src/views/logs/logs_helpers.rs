//! Parte de `views::logs` (extraída para manter os arquivos revisáveis).

use super::*;

/// Card de absorção com densidade por tempo (barras do PNG, dados reais).
pub(super) fn absorption_card(ui: &mut egui::Ui, rate: Option<f64>, entries: &[LogEntry]) {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(theme::SPACE_LG)
        .stroke(egui::Stroke::new(
            1.0,
            theme::tint(theme::PRIMARY_FIXED_DIM, 45),
        ))
        .show(ui, |ui| {
            ui.set_min_width(ui.available_width());
            ui.set_min_height(104.0);
            ui.label(
                egui::RichText::new("TAXA DE ABSORÇÃO")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.label(
                egui::RichText::new(rate.map_or(String::from("—"), |r| format!("{r:.1} ev/s")))
                    .size(42.0)
                    .strong()
                    .color(theme::PRIMARY_FIXED_DIM),
            );
            density_bars(ui, entries);
        });
}

/// 12 barras de densidade por duodécimo do span (painter puro, sem nós).
pub(super) fn density_bars(ui: &mut egui::Ui, entries: &[LogEntry]) {
    const BARS: usize = 12;
    let mut buckets = [0u32; BARS];
    let span: u64 = entries.iter().map(|entry| entry.offset_ms).sum();
    if span == 0 {
        if !entries.is_empty() {
            buckets[BARS - 1] = entries.len() as u32;
        }
    } else {
        let mut elapsed = 0u64;
        for entry in entries {
            let at = (elapsed * BARS as u64)
                .checked_div(span)
                .unwrap_or(0)
                .min(BARS as u64 - 1) as usize;
            buckets[at] += 1;
            elapsed += entry.offset_ms;
        }
    }
    let max = buckets.iter().max().copied().unwrap_or(0).max(1);
    let (rect, _) =
        ui.allocate_exact_size(egui::vec2(ui.available_width(), 24.0), egui::Sense::hover());
    let slot = rect.width() / BARS as f32;
    for (index, &count) in buckets.iter().enumerate() {
        let height = (count as f32 / max as f32) * rect.height();
        let bar = egui::Rect::from_min_size(
            egui::pos2(
                rect.left() + index as f32 * slot + 1.0,
                rect.bottom() - height,
            ),
            egui::vec2(slot - 2.0, height.max(1.0)),
        );
        ui.painter()
            .rect_filled(bar, 2.0, theme::tint(theme::PRIMARY_FIXED_DIM, 70));
    }
}

/// Offsets relativos ao mais novo (ms, ≤ 0): `rel[i] = -(t[n-1] − t[i])`.
pub(super) fn rel_offsets(entries: &[LogEntry]) -> Vec<i64> {
    let mut out = vec![0i64; entries.len()];
    let mut acc: i64 = 0;
    for (index, entry) in entries.iter().enumerate().rev() {
        out[index] = -acc;
        acc += entry.offset_ms as i64;
    }
    out
}

/// Relativo de um slot (para o inspetor; 0 se ausente).
pub(super) fn rel_of(entries: &[LogEntry], slot: u64) -> i64 {
    let Some(pos) = entries.iter().position(|entry| entry.slot == slot) else {
        return 0;
    };
    -(entries[pos + 1..]
        .iter()
        .map(|entry| entry.offset_ms as i64)
        .sum::<i64>())
}

/// Offset adaptativo (espelho do PNG; ms inteiros — sem fração no fio).
pub(super) fn fmt_offset(ms: i64) -> String {
    if ms >= 0 {
        String::from("+0ms")
    } else if -ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.2}s", ms as f64 / 1000.0)
    }
}

/// Dump completo da entrada para `COPIAR DUMP`.
pub(super) fn dump(entry: &LogEntry) -> String {
    format!(
        "SLOT #{}\n{} [{}] [{}]\nthread: {}\nfonte: {}\n\n{}\n\ncontexto: {}\n\nstack:\n{}",
        entry.slot,
        entry.timestamp,
        entry.level,
        entry.source,
        entry.thread,
        entry.origin,
        entry.message,
        entry.payload.as_deref().unwrap_or("(sem contexto)"),
        entry.backtrace.as_deref().unwrap_or("(stack só em ERRO)"),
    )
}

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
