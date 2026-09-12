//! Interação da galeria (UI-1.3): renderiza nos dois temas, localiza
//! componentes por rótulo acessível e exercita um clique sem pânico.
//! Headless via egui_kittest; janela real pertence a UI-6.

use egui_kittest::{kittest::Queryable as _, Harness};
use orchestrator_studio::design::ResolvedTheme;
use orchestrator_studio::gallery::GalleryState;

#[test]
fn gallery_renders_both_themes_with_named_components() {
    for theme in [ResolvedTheme::Dark, ResolvedTheme::Light] {
        let mut harness = Harness::new_ui_state(
            |ui, state: &mut GalleryState| orchestrator_studio::gallery::show(ui, state),
            GalleryState::new(theme),
        );
        harness.run();
        harness.get_by_label("Galeria de componentes");
        harness.get_by_label("Ação primária");
        harness.get_by_label("Capacidade indisponível");
        assert_eq!(harness.state().theme, theme);
    }
}

#[test]
fn gallery_primary_button_accepts_click() {
    let mut harness = Harness::new_ui_state(
        |ui, state: &mut GalleryState| orchestrator_studio::gallery::show(ui, state),
        GalleryState::new(ResolvedTheme::Dark),
    );
    harness.run();
    harness.get_by_label("Ação primária").click();
    harness.run();
}
