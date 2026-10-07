//! Design system do Studio (migração Stitch → egui, T-890-03 v6).
//!
//! Fonte da verdade: `stitch_dds_orchestrator_studio_ux_redesign/dds_
//! orchestrator_studio/DESIGN.md` + `tailwind.config` embutido nos mockups
//! (tokens verificados 1:1). Personalidade: "High-Density Industrial
//! Observability" — profundidade por LUMINÂNCIA (sem sombras) e hairlines
//! de 1 px.
//!
//! Aplicado UMA vez no boot via [`apply`] (`cc.egui_ctx`).

use eframe::egui;
use egui::{Color32, FontDefinitions, FontFamily, TextStyle};

// ── Superfícies (escada de luminância fria) ─────────────────────────────
/// Canvas/canvas de painéis.
pub const SURFACE_LOWEST: Color32 = Color32::from_rgb(0x09, 0x0f, 0x15);
/// Linha base do shell.
pub const SURFACE: Color32 = Color32::from_rgb(0x0f, 0x14, 0x1b);
/// Hover/linhas zebradas alternadas.
pub const SURFACE_LOW: Color32 = Color32::from_rgb(0x17, 0x1c, 0x23);
/// Cards.
pub const SURFACE_CONTAINER: Color32 = Color32::from_rgb(0x1b, 0x20, 0x27);
/// Headers de tabela/chips/hover forte.
pub const SURFACE_HIGH: Color32 = Color32::from_rgb(0x25, 0x2a, 0x32);
/// Bordas 1 px.
pub const SURFACE_HIGHEST: Color32 = Color32::from_rgb(0x30, 0x35, 0x3d);
/// Borda de foco.
pub const SURFACE_BRIGHT: Color32 = Color32::from_rgb(0x34, 0x39, 0x41);

// ── Texto ───────────────────────────────────────────────────────────────
pub const ON_SURFACE: Color32 = Color32::from_rgb(0xde, 0xe2, 0xec);
pub const ON_SURFACE_VARIANT: Color32 = Color32::from_rgb(0xba, 0xc9, 0xcc);
pub const OUTLINE: Color32 = Color32::from_rgb(0x84, 0x93, 0x96);
pub const OUTLINE_VARIANT: Color32 = Color32::from_rgb(0x3b, 0x49, 0x4c);

// ── Accent (família ciano de telemetria) ────────────────────────────────
/// O accent do produto — item de navegação ativo, foco, botão primário.
pub const PRIMARY_CONTAINER: Color32 = Color32::from_rgb(0x00, 0xe5, 0xff);
pub const ON_PRIMARY: Color32 = Color32::from_rgb(0x00, 0x36, 0x3d);
pub const PRIMARY_FIXED_DIM: Color32 = Color32::from_rgb(0x00, 0xda, 0xf3);
pub const SECONDARY_FIXED: Color32 = Color32::from_rgb(0xb3, 0xeb, 0xff);

// ── Semântica de estado (com tinta de 10% no fundo dos badges) ──────────
/// Fonte viva com dados.
pub const OK: Color32 = Color32::from_rgb(0x10, 0xb9, 0x81);
/// Degradado/parcial (auto-carga, presença sem prova).
pub const WARN: Color32 = Color32::from_rgb(0xf5, 0x9e, 0x0b);
/// Falha/DESVIADO.
pub const ERROR: Color32 = Color32::from_rgb(0xef, 0x44, 0x44);
/// 401 — estado de CREDENCIAL, distinto de falha de rede.
pub const AUTH: Color32 = Color32::from_rgb(0xa8, 0x55, 0xf7);
/// Apagado/never-loaded.
pub const STALE: Color32 = Color32::from_rgb(0x6b, 0x72, 0x80);

// ── Espaçamento (escala do DESIGN.md) ───────────────────────────────────
pub const SPACE_XS: f32 = 2.0;
pub const SPACE_SM: f32 = 4.0;
pub const SPACE_MD: f32 = 8.0;
pub const SPACE_LG: f32 = 12.0;
pub const SPACE_XL: f32 = 16.0;

/// Raio padrão de badges/botões/cards (2 px — "industrial").
pub const RADIUS_SM: f32 = 2.0;
pub const RADIUS_MD: f32 = 4.0;
pub const RADIUS_LG: f32 = 8.0;

/// Tinta translúcida para fundo de badge (10% da cor, per DESIGN.md).
#[must_use]
pub fn tint(color: Color32, alpha10: u8) -> Color32 {
    Color32::from_rgba_unmultiplied(color.r(), color.g(), color.b(), alpha10 * 10)
}

/// Aplica o tema completo ao contexto: Visuals + FontDefinitions + TextStyles.
pub fn apply(ctx: &egui::Context) {
    apply_fonts(ctx);
    apply_visuals(ctx);
}

/// Fontes: Inter (UI) + JetBrains Mono (dados) embutidas; glifos
/// (●◐◌🛰🛡) preservados via fallback para as fontes de emoji do egui.
fn apply_fonts(ctx: &egui::Context) {
    let mut fonts = FontDefinitions::default();

    fonts.font_data.insert(
        "inter".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/Inter.ttf"
        ))),
    );
    fonts.font_data.insert(
        "jetbrains_mono".into(),
        std::sync::Arc::new(egui::FontData::from_static(include_bytes!(
            "../assets/fonts/JetBrainsMono.ttf"
        ))),
    );

    // Proportional: Inter primeiro; Ubuntu-Light e NotoEmoji ficam como
    // fallback para glifos que a Inter não cobre (●◐◌→🛰🛡✔✘).
    if let Some(proportional) = fonts.families.get_mut(&FontFamily::Proportional) {
        let fallbacks = proportional.clone();
        proportional.clear();
        proportional.push("inter".into());
        proportional.extend(fallbacks);
    }
    // Monospace: JetBrains Mono primeiro (dados/hex/timestamps), emoji fallback.
    if let Some(mono) = fonts.families.get_mut(&FontFamily::Monospace) {
        let fallbacks = mono.clone();
        mono.clear();
        mono.insert(0, "jetbrains_mono".into());
        mono.extend(fallbacks);
    }

    // Escala de TextStyles (pt): tokens do DESIGN.md com o Inter como família.
    ctx.all_styles_mut(|style| {
        style.text_styles = [
            (TextStyle::Heading, FontFamily::Proportional, 16.0),
            (TextStyle::Body, FontFamily::Proportional, 13.0),
            (TextStyle::Button, FontFamily::Proportional, 12.0),
            (TextStyle::Small, FontFamily::Proportional, 11.0),
            (TextStyle::Monospace, FontFamily::Monospace, 11.0),
        ]
        .into_iter()
        .map(|(key, family, size)| (key, egui::FontId::new(size, family)))
        .collect();
    });
    ctx.set_fonts(fonts);
}

/// Visuals: superfícies da escada de luminância, hairlines, rounding 2/4.
fn apply_visuals(ctx: &egui::Context) {
    ctx.all_styles_mut(|style| {
        let v = &mut style.visuals;
        v.dark_mode = true;
        v.override_text_color = Some(ON_SURFACE);

        // Painéis: canvas LOWEST; janelas/cards CONTAINER.
        v.panel_fill = SURFACE_LOWEST;
        v.window_fill = SURFACE_CONTAINER;
        v.extreme_bg_color = SURFACE_LOW;

        // Hairlines 1 px em vez de sombras (DESIGN.md: sem drop shadows).
        v.widgets.noninteractive.bg_stroke = egui::Stroke::new(1.0, SURFACE_HIGHEST);
        v.widgets.inactive.bg_stroke = egui::Stroke::new(1.0, SURFACE_HIGHEST);
        v.widgets.hovered.bg_stroke = egui::Stroke::new(1.0, PRIMARY_FIXED_DIM);
        v.widgets.active.bg_stroke = egui::Stroke::new(1.0, PRIMARY_CONTAINER);

        v.widgets.noninteractive.bg_fill = SURFACE_LOW;
        v.widgets.inactive.bg_fill = SURFACE_CONTAINER;
        v.widgets.hovered.bg_fill = SURFACE_HIGH;
        v.widgets.active.bg_fill = SURFACE_HIGH;

        v.widgets.inactive.fg_stroke = egui::Stroke::new(1.0, ON_SURFACE);
        v.widgets.hovered.fg_stroke = egui::Stroke::new(1.0, ON_SURFACE);
        v.widgets.active.fg_stroke = egui::Stroke::new(1.0, PRIMARY_CONTAINER);
        v.widgets.noninteractive.fg_stroke = egui::Stroke::new(1.0, ON_SURFACE_VARIANT);

        // Rounding 2/4 (industrial). CornerRadius é u8 por canto.
        let small = egui::CornerRadius::same(RADIUS_SM as u8);
        v.widgets.noninteractive.corner_radius = small;
        v.widgets.inactive.corner_radius = small;
        v.widgets.hovered.corner_radius = egui::CornerRadius::same(RADIUS_MD as u8);
        v.widgets.active.corner_radius = small;
        v.window_corner_radius = egui::CornerRadius::same(RADIUS_MD as u8);

        // Seleção = accent ciano.
        v.selection.bg_fill = PRIMARY_CONTAINER;
        v.selection.stroke = egui::Stroke::new(1.0, ON_PRIMARY);

        v.hyperlink_color = PRIMARY_FIXED_DIM;
    });
}
