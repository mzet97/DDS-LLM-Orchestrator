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
    let show_frozen = panel.paused;
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
    // Faixa de stream (espelho do PNG; tudo medido do buffer).
    ui.horizontal(|ui| {
        kit::badge(ui, "FIFO RING BUFFER [500]", theme::PRIMARY_FIXED_DIM);
        ui.label(
            egui::RichText::new(format!(
                "STREAM: {} · Taxa {} · RETENÇÃO: {retention}",
                if panel.paused {
                    "PAUSADO (foto)"
                } else {
                    "REALTIME ATIVO"
                },
                rate.map_or(String::from("—"), |r| format!("~{r:.1} logs/s")),
            ))
            .monospace()
            .small()
            .color(theme::ON_SURFACE_VARIANT),
        );
    });
    ui.add_space(theme::SPACE_XS);

    // ── 4 KPIs ──
    let warn_count = live.iter().filter(|e| e.level == "WARN").count();
    let error_count = live.iter().filter(|e| e.level == "ERRO").count();
    let pct = live.len() * 100 / studio_log::CAPACITY;
    let share = |n: usize| {
        if live.is_empty() {
            0.0
        } else {
            n as f64 * 100.0 / live.len() as f64
        }
    };
    ui.columns(4, |cols| {
        kit::metric_card(
            &mut cols[0],
            "Ocupação do ring buffer",
            format!("{}/500", live.len()),
            &format!("{pct}% cheio · {evicted} descartados"),
            theme::PRIMARY_FIXED_DIM,
        );
        absorption_card(&mut cols[1], rate, &live);
        kit::metric_card(
            &mut cols[2],
            "Severidade de alerta",
            // Valor HERO curto (achado 3.7: "N Falhas Ativas" quebra).
            format!("{error_count}"),
            &format!(
                "Falhas Ativas · {:.1}% ERRO · {:.1}% WARN",
                share(error_count),
                share(warn_count)
            ),
            if error_count > 0 {
                theme::ERROR
            } else if warn_count > 0 {
                theme::WARN
            } else {
                theme::OK
            },
        );
        kit::metric_card(
            &mut cols[3],
            "Sink stderr / TTY",
            String::from("SINC"),
            &if evicted == 0 {
                String::from("/dev/stderr · SEM DROP")
            } else {
                format!("/dev/stderr · {evicted} descartados")
            },
            theme::OK,
        );
    });
    ui.add_space(theme::SPACE_MD);

    // Lista exibida: congelada sob pausa (slide do buffer segue vivo).
    let entries: &[LogEntry] = if show_frozen { &panel.frozen } else { &live };
    // Ações comutadas na toolbar, aplicadas após o bloco de leitura.
    let mut toggle_pause = false;
    let mut do_clear = false;
    // ── Toolbar: filtros + busca + toggles + ações ──
    ui.horizontal_wrapped(|ui| {
        if ui
            .selectable_label(
                panel.filter_level.is_none(),
                egui::RichText::new(format!("Todos ({})", entries.len()))
                    .monospace()
                    .small(),
            )
            .clicked()
        {
            panel.filter_level = None;
        }
        for level in LEVELS {
            let count = entries.iter().filter(|e| e.level == level).count();
            if ui
                .selectable_label(
                    panel.filter_level == Some(level),
                    egui::RichText::new(format!("{level} ({count})"))
                        .monospace()
                        .small()
                        .color(level_color(level)),
                )
                .clicked()
            {
                panel.filter_level = Some(level);
            }
        }
        ui.separator();
        ui.add(
            egui::TextEdit::singleline(&mut panel.search)
                .hint_text("filtro rápido (substring)…")
                .desired_width(240.0)
                .font(egui::TextStyle::Monospace),
        );
        ui.separator();
        ui.checkbox(&mut panel.auto_scroll, "Auto-scroll");
        if ui.checkbox(&mut panel.mirror, "Espelhar stderr").changed() {
            studio_log::set_mirror(panel.mirror);
        }
        if ui
            .button(if panel.paused { "Retomar" } else { "Pausar" })
            .clicked()
        {
            toggle_pause = true;
        }
        if ui.button("Limpar buffer").clicked() {
            do_clear = true;
        }
        if ui.button("Exportar (.log)").clicked() {
            match studio_log::export() {
                Ok(path) => {
                    studio_log::info(format!("logs exportados em {}", path.display()));
                }
                Err(err) => studio_log::error(format!("falha ao exportar logs: {err}")),
            }
        }
        if panel.paused {
            kit::badge(ui, "FOTO CONGELADA", theme::WARN);
        }
    });
    ui.add_space(theme::SPACE_SM);

    // ── Tabela invertida (novo primeiro, espelho do PNG) ──
    let search = panel.search.trim().to_lowercase();
    let offsets = rel_offsets(entries);
    let rows: Vec<(usize, &LogEntry)> = entries
        .iter()
        .enumerate()
        .filter(|(_, entry)| {
            let level_ok = panel.filter_level.is_none_or(|level| entry.level == level);
            let search_ok = search.is_empty()
                || entry.message.to_lowercase().contains(&search)
                || entry.source.to_lowercase().contains(&search);
            level_ok && search_ok
        })
        .collect();
    let selected_slot = panel.selected;
    let mut clicked: Option<u64> = None;
    let mut scroll = egui::ScrollArea::vertical()
        .auto_shrink([false, false])
        .max_height(380.0)
        .id_salt("studio_logs_scroll");
    // Novo no topo: auto-scroll trava no offset 0 (tail invertido).
    if panel.auto_scroll && !panel.paused {
        scroll = scroll.vertical_scroll_offset(0.0);
    }
    scroll.show(ui, |ui| {
        if rows.is_empty() {
            kit::empty_state(ui, "(nenhum evento no filtro atual)");
            return;
        }
        kit::table("studio_logs").show(ui, |ui| {
            kit::grid_header(
                ui,
                &[
                    "#L SLOT",
                    "TIMESTAMP",
                    "NÍVEL",
                    "SUBSISTEMA",
                    "CARGA ÚTIL DO EVENTO",
                    "OFFSET",
                ],
            );
            for (index, entry) in rows.iter().rev() {
                let is_selected = selected_slot == Some(entry.slot);
                if ui
                    .selectable_label(
                        is_selected,
                        egui::RichText::new(entry.slot.to_string())
                            .monospace()
                            .small()
                            .color(theme::OUTLINE),
                    )
                    .clicked()
                {
                    clicked = Some(entry.slot);
                }
                kit::mono_cell(ui, &entry.timestamp);
                kit::badge(ui, entry.level, level_color(entry.level));
                kit::mono_cell(ui, &format!("[{}]", entry.source));
                ui.label(&entry.message);
                kit::num_cell(ui, &fmt_offset(offsets[*index]));
                ui.end_row();
            }
        });
    });
    if toggle_pause {
        panel.paused = !panel.paused;
        if panel.paused {
            panel.frozen = live.clone();
        } else {
            panel.frozen.clear();
        }
    }
    if do_clear {
        studio_log::clear();
        panel.frozen.clear();
        panel.selected = None;
    } else if let Some(slot) = clicked {
        panel.selected = (panel.selected != Some(slot)).then_some(slot);
    }

    // ── Inspetor forense da entrada selecionada ──
    let shown: &[LogEntry] = if panel.paused { &panel.frozen } else { &live };
    if let Some(slot) = panel.selected {
        if let Some(entry) = shown.iter().find(|e| e.slot == slot) {
            ui.add_space(theme::SPACE_SM);
            section(ui, &format!("INSPETOR DIAGNÓSTICO · SLOT #{}", entry.slot));
            kit::accent_card(ui, level_color(entry.level), |ui| {
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "THREAD: {} · FONTE: {}",
                            entry.thread, entry.origin
                        ))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                    ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new("fechar").monospace().small(),
                            ))
                            .clicked()
                        {
                            panel.selected = None;
                        }
                        if ui
                            .add(egui::Button::new(
                                egui::RichText::new("COPIAR DUMP").monospace().small(),
                            ))
                            .clicked()
                        {
                            ui.ctx().copy_text(dump(entry));
                        }
                    });
                });
                ui.horizontal(|ui| {
                    ui.label(
                        egui::RichText::new(format!(
                            "SUBSISTEMA: [{}] · MOMENTO: {} · OFFSET: {}",
                            entry.source,
                            entry.timestamp,
                            fmt_offset(rel_of(shown, entry.slot))
                        ))
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                    );
                });
                ui.add_space(theme::SPACE_XS);
                ui.label(header("RESUMO DA MENSAGEM RAW"));
                ui.add(
                    egui::TextEdit::multiline(&mut entry.message.clone())
                        .font(egui::TextStyle::Monospace)
                        .desired_rows(2)
                        .interactive(false),
                );
                if let Some(payload) = &entry.payload {
                    ui.add_space(theme::SPACE_XS);
                    ui.label(header("CONTEXTO SERIALIZADO DO NÓ (JSON)"));
                    ui.add(
                        egui::TextEdit::multiline(&mut payload.clone())
                            .font(egui::TextStyle::Monospace)
                            .desired_rows(3)
                            .interactive(false),
                    );
                }
                if let Some(backtrace) = &entry.backtrace {
                    ui.add_space(theme::SPACE_XS);
                    ui.label(header("CALL STACK TRACE (RUST · CAPTURADO NO REGISTRO)"));
                    egui::ScrollArea::vertical()
                        .id_salt("log-backtrace")
                        .max_height(180.0)
                        .show(ui, |ui| {
                            ui.add(
                                egui::TextEdit::multiline(&mut backtrace.clone())
                                    .font(egui::TextStyle::Monospace)
                                    .desired_rows(6)
                                    .interactive(false),
                            );
                        });
                }
            });
        }
    }
}

/// Card de absorção com densidade por tempo (barras do PNG, dados reais).
fn absorption_card(ui: &mut egui::Ui, rate: Option<f64>, entries: &[LogEntry]) {
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
fn density_bars(ui: &mut egui::Ui, entries: &[LogEntry]) {
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
fn rel_offsets(entries: &[LogEntry]) -> Vec<i64> {
    let mut out = vec![0i64; entries.len()];
    let mut acc: i64 = 0;
    for (index, entry) in entries.iter().enumerate().rev() {
        out[index] = -acc;
        acc += entry.offset_ms as i64;
    }
    out
}

/// Relativo de um slot (para o inspetor; 0 se ausente).
fn rel_of(entries: &[LogEntry], slot: u64) -> i64 {
    let Some(pos) = entries.iter().position(|entry| entry.slot == slot) else {
        return 0;
    };
    -(entries[pos + 1..]
        .iter()
        .map(|entry| entry.offset_ms as i64)
        .sum::<i64>())
}

/// Offset adaptativo (espelho do PNG; ms inteiros — sem fração no fio).
fn fmt_offset(ms: i64) -> String {
    if ms >= 0 {
        String::from("+0ms")
    } else if -ms < 1000 {
        format!("{ms}ms")
    } else {
        format!("{:.2}s", ms as f64 / 1000.0)
    }
}

/// Dump completo da entrada para `COPIAR DUMP`.
fn dump(entry: &LogEntry) -> String {
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
fn header(text: &str) -> egui::RichText {
    egui::RichText::new(text)
        .monospace()
        .small()
        .strong()
        .color(theme::OUTLINE)
}

/// Seção com hairline, título à esquerda (espelho do PNG).
fn section(ui: &mut egui::Ui, text: &str) {
    ui.label(header(text));
    ui.separator();
    ui.add_space(theme::SPACE_SM);
}

#[cfg(test)]
mod tests {
    use super::{fmt_offset, rel_offsets};
    use crate::studio_log::LogEntry;

    fn entry(offset_ms: u64) -> LogEntry {
        LogEntry {
            timestamp: String::from("00:00:00"),
            level: "INFO",
            source: "GUI",
            message: String::new(),
            payload: None,
            origin: String::new(),
            thread: String::new(),
            offset_ms,
            backtrace: None,
            slot: 0,
        }
    }

    #[test]
    fn rel_offsets_anchor_newest_at_zero() {
        let entries = vec![entry(0), entry(2700), entry(100)];
        assert_eq!(rel_offsets(&entries), vec![-2800, -100, 0]);
        assert!(rel_offsets(&[]).is_empty());
    }

    #[test]
    fn fmt_offset_matches_design_units() {
        assert_eq!(fmt_offset(0), "+0ms");
        assert_eq!(fmt_offset(-270), "-270ms");
        assert_eq!(fmt_offset(-2700), "-2.70s");
    }
}
