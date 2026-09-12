//! Fundação visual do Studio (UI-1.1): tokens, temas e contraste.
//!
//! Paleta proposta pelo SDD (§7); pares efetivos verificados por
//! [`contrast_ratio`] (meta 4,5:1 para texto, UI-6.2).

/// Tema de aparência (T13: Sistema / Claro / Escuro).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Default)]
pub enum Theme {
    /// Segue o sistema quando detectável.
    #[default]
    System,
    Light,
    Dark,
}

impl Theme {
    /// Rótulo em pt-BR para preferências.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::System => "Sistema",
            Self::Light => "Claro",
            Self::Dark => "Escuro",
        }
    }

    /// Resolve o tema efetivo (modo claro quando o sistema é desconhecido).
    #[must_use]
    pub const fn resolve(self, system_dark: Option<bool>) -> ResolvedTheme {
        match self {
            Self::Dark => ResolvedTheme::Dark,
            Self::Light => ResolvedTheme::Light,
            Self::System => {
                if matches!(system_dark, Some(true)) {
                    ResolvedTheme::Dark
                } else {
                    ResolvedTheme::Light
                }
            }
        }
    }
}

/// Tema efetivo após resolução.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ResolvedTheme {
    Light,
    Dark,
}

/// Cor RGB de 8 bits por canal (independente de toolkit, testável).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Rgb(pub u8, pub u8, pub u8);

/// Par texto/fundo para verificação de contraste.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct TextPair {
    pub text: Rgb,
    pub background: Rgb,
}

/// Razão de contraste WCAG (1–21) entre texto e fundo.
#[must_use]
pub fn contrast_ratio(pair: TextPair) -> f64 {
    fn luminance(channel: u8) -> f64 {
        let linear = f64::from(channel) / 255.0;
        if linear <= 0.03928 {
            linear / 12.92
        } else {
            ((linear + 0.055) / 1.055).powf(2.4)
        }
    }
    fn relative_luminance(color: Rgb) -> f64 {
        0.2126 * luminance(color.0) + 0.7152 * luminance(color.1) + 0.0722 * luminance(color.2)
    }
    let lighter = relative_luminance(pair.text).max(relative_luminance(pair.background));
    let darker = relative_luminance(pair.text).min(relative_luminance(pair.background));
    (lighter + 0.05) / (darker + 0.05)
}

/// Tokens semânticos de cor (§7). Valores dark/light da tabela do SDD.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub struct Palette {
    pub background: Rgb,
    pub surface: Rgb,
    pub surface_raised: Rgb,
    pub text_primary: Rgb,
    pub text_secondary: Rgb,
    pub border_subtle: Rgb,
    pub border_control: Rgb,
    pub accent: Rgb,
    pub on_accent: Rgb,
    pub success_text: Rgb,
    pub warning_text: Rgb,
    pub danger_text: Rgb,
}

/// Paleta efetiva do tema resolvido.
#[must_use]
pub const fn palette(theme: ResolvedTheme) -> Palette {
    match theme {
        ResolvedTheme::Dark => Palette {
            background: Rgb(0x11, 0x13, 0x18),
            surface: Rgb(0x19, 0x1D, 0x25),
            surface_raised: Rgb(0x21, 0x27, 0x33),
            text_primary: Rgb(0xF2, 0xF4, 0xF8),
            text_secondary: Rgb(0xAD, 0xB8, 0xC9),
            border_subtle: Rgb(0x34, 0x3E, 0x4E),
            border_control: Rgb(0x6B, 0x78, 0x8C),
            accent: Rgb(0x79, 0xA9, 0xFF),
            on_accent: Rgb(0x11, 0x13, 0x18),
            success_text: Rgb(0x78, 0xD4, 0xAA),
            warning_text: Rgb(0xF2, 0xC6, 0x6D),
            danger_text: Rgb(0xFF, 0x94, 0x9C),
        },
        ResolvedTheme::Light => Palette {
            background: Rgb(0xF4, 0xF6, 0xFA),
            surface: Rgb(0xFF, 0xFF, 0xFF),
            surface_raised: Rgb(0xEA, 0xF0, 0xF8),
            text_primary: Rgb(0x18, 0x20, 0x2D),
            text_secondary: Rgb(0x4D, 0x5C, 0x70),
            border_subtle: Rgb(0xD6, 0xDE, 0xE9),
            border_control: Rgb(0x72, 0x7F, 0x92),
            accent: Rgb(0x20, 0x5A, 0xD2),
            on_accent: Rgb(0xFF, 0xFF, 0xFF),
            success_text: Rgb(0x19, 0x6B, 0x45),
            warning_text: Rgb(0x80, 0x58, 0x00),
            danger_text: Rgb(0xB4, 0x23, 0x3B),
        },
    }
}

/// Aplica a paleta aos visuais do egui (fino: só apresentação).
pub fn apply(ctx: &egui::Context, theme: ResolvedTheme) {
    let colors = palette(theme);
    let to_egui = |color: Rgb| egui::Color32::from_rgb(color.0, color.1, color.2);
    let mut visuals = match theme {
        ResolvedTheme::Dark => egui::Visuals::dark(),
        ResolvedTheme::Light => egui::Visuals::light(),
    };
    visuals.panel_fill = to_egui(colors.background);
    visuals.window_fill = to_egui(colors.surface);
    visuals.faint_bg_color = to_egui(colors.surface_raised);
    visuals.override_text_color = Some(to_egui(colors.text_primary));
    visuals.weak_text_color = Some(to_egui(colors.text_secondary));
    ctx.set_visuals(visuals);
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn system_defaults_to_light_when_unknown() {
        assert_eq!(Theme::System.resolve(None), ResolvedTheme::Light);
        assert_eq!(Theme::System.resolve(Some(true)), ResolvedTheme::Dark);
        assert_eq!(Theme::System.resolve(Some(false)), ResolvedTheme::Light);
    }

    #[test]
    fn primary_text_meets_contrast_on_both_themes() {
        for theme in [ResolvedTheme::Dark, ResolvedTheme::Light] {
            let colors = palette(theme);
            let ratio = contrast_ratio(TextPair {
                text: colors.text_primary,
                background: colors.background,
            });
            assert!(ratio >= 4.5, "{theme:?}: {ratio:.2}");
        }
    }
}
