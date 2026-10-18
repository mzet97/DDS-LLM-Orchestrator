//! Parte de `views::machines` (extraída para manter os arquivos revisáveis).

use super::*;

pub(super) const LEASE_SECS: f32 = 10.0;
/// Período do heartbeat de presença (s) — ciclo de varredura do hero.
pub(super) const HEARTBEAT_SECS: u64 = 5;

/// Cor do ponto de status por estado da sonda (ciano = autenticado, roxo =
/// falha de auth, vermelho = offline — legenda do mockup).
pub(super) fn dot_color(state: ProbeState) -> egui::Color32 {
    match state {
        ProbeState::Online => theme::PRIMARY_FIXED_DIM,
        ProbeState::Offline => theme::ERROR,
        ProbeState::AuthPending => theme::AUTH,
        ProbeState::Unknown => theme::STALE,
    }
}

/// Rótulo do heartbeat pela idade (lease 10 s).
pub(super) fn heartbeat_label(age_secs: u64) -> (&'static str, egui::Color32) {
    if age_secs <= 5 {
        ("● FRESCO", theme::PRIMARY_FIXED_DIM)
    } else if age_secs <= 10 {
        ("● VIVO", theme::PRIMARY_FIXED_DIM)
    } else {
        ("○ EXPIRANDO", theme::STALE)
    }
}

/// Idade fracionária do último heartbeat (s) — "5.1s atrás" do mockup.
pub(super) fn age_frac(node: &DiscoveredNode, now_ns: u64) -> f32 {
    now_ns.saturating_sub(node.last_seen_unix_ns) as f32 / 1_000_000_000.0
}

/// Trunca texto com "…" (node ids longos da coluna 1).
pub(super) fn truncate(text: &str, max_chars: usize) -> String {
    if text.chars().count() <= max_chars {
        return text.to_owned();
    }
    let mut out: String = text.chars().take(max_chars.saturating_sub(1)).collect();
    out.push('…');
    out
}

/// "Token recusado" vs "Token ausente" pelo detalhe do probe 401.
pub(super) fn auth_curto(detail: &str) -> &'static str {
    if detail.contains("recusado") {
        "Token recusado"
    } else {
        "Token ausente"
    }
}

/// Porta da URL (`http://host:porta[/...]`) — linha "porta recusada" honesta.
pub(super) fn port_of(url: &str) -> Option<&str> {
    let after_scheme = url.split("://").nth(1).unwrap_or(url);
    let authority = after_scheme.split('/').next().unwrap_or_default();
    let port = authority.rsplit(':').next().unwrap_or_default();
    (!port.is_empty() && port.chars().all(|c| c.is_ascii_digit())).then_some(port)
}

/// Moldura dos cards da 3.10 (mesmo idioma dos `TeleCard` da 3.1).
pub(super) fn card_frame() -> egui::Frame {
    egui::Frame::NONE
        .fill(theme::SURFACE_CONTAINER)
        .stroke(egui::Stroke::new(1.0, theme::SURFACE_HIGHEST))
        .corner_radius(egui::CornerRadius::same(theme::RADIUS_MD as u8))
        .inner_margin(egui::Margin::same(10))
}

/// Cabeçalho de card: título à esquerda + etiqueta à direita.
pub(super) fn card_title(ui: &mut egui::Ui, title: &str, tag: &str) {
    ui.horizontal(|ui| {
        ui.label(
            egui::RichText::new(title)
                .monospace()
                .size(11.5)
                .strong()
                .color(theme::ON_SURFACE),
        );
        ui.with_layout(egui::Layout::right_to_left(egui::Align::Center), |ui| {
            ui.label(
                egui::RichText::new(tag)
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
        });
    });
}

/// Stat do hero (rótulo + valores): caixa fixa com layout `top_down`
/// explícito — imune à herança de direção do `right_to_left` pai.
pub(super) fn hero_stat(
    ui: &mut egui::Ui,
    width: f32,
    label: &str,
    values: impl FnOnce(&mut egui::Ui),
) {
    ui.allocate_ui_with_layout(
        egui::vec2(width, 48.0),
        egui::Layout::top_down(egui::Align::LEFT),
        |ui| {
            ui.label(
                egui::RichText::new(label)
                    .monospace()
                    .size(9.5)
                    .color(theme::OUTLINE),
            );
            values(ui);
        },
    );
}
