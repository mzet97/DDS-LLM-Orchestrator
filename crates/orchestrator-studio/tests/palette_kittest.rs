//! Teclado da paleta (UI-6.1): abrir, filtrar digitando, Enter ativa
//! item seguro e Esc fecha com foco preservado. Headless; janela real
//! e leitor de tela pertencem ao roteiro manual.

use egui::accesskit::Role;
use egui_kittest::{
    kittest::{AccessKitNode, Queryable as _},
    Harness,
};
use orchestrator_studio::shell::{filter_commands, CommandItem, ShellContext};

/// Campo de busca da paleta (único TextInput da janela aberta).
fn search_field<'tree>(harness: &'tree Harness<'tree, PaletteState>) -> egui_kittest::Node<'tree> {
    let mut all = harness.get_all_by(|node: &AccessKitNode<'_>| node.role() == Role::TextInput);
    all.next().expect("busca existe")
}

fn catalog() -> Vec<CommandItem> {
    vec![
        CommandItem {
            title: String::from("Ir para Agentes"),
            section: String::from("Navegação"),
            dangerous: false,
        },
        CommandItem {
            title: String::from("Ir para Revisão"),
            section: String::from("Navegação"),
            dangerous: false,
        },
    ]
}

struct PaletteState {
    ctx: ShellContext,
    open: bool,
    query: String,
    picked: Vec<String>,
}

#[test]
fn palette_type_filters_enter_picks_and_esc_closes() {
    let items = catalog();
    let mut harness = Harness::new_ui_state(
        |ui, state: &mut PaletteState| {
            orchestrator_studio::shell::show_top_bar(ui, &mut state.ctx, &mut state.open);
            if state.open {
                let entries = items.clone();
                if let Some(picked) = orchestrator_studio::shell::show_palette(
                    ui,
                    &mut state.open,
                    &mut state.query,
                    &entries,
                ) {
                    state.picked.push(picked.title);
                }
            }
        },
        PaletteState {
            ctx: ShellContext::default(),
            open: false,
            query: String::new(),
            picked: Vec::new(),
        },
    );
    harness.run();
    harness.get_by_label("Buscar / Ctrl+K").click();
    harness.run();
    assert!(harness.state().open);
    search_field(&harness).click();
    harness.run();
    search_field(&harness).type_text("Ag");
    harness.run();
    let ranked = filter_commands(&items, harness.state().query.as_str());
    assert_eq!(ranked.len(), 1);
    assert_eq!(ranked[0].title, "Ir para Agentes");
    harness.key_press(egui::Key::Enter);
    harness.run();
    assert_eq!(
        harness.state().picked,
        vec![String::from("Ir para Agentes")]
    );
    assert!(!harness.state().open);
}

#[test]
fn palette_esc_closes_without_picking() {
    let items = catalog();
    let mut harness = Harness::new_ui_state(
        |ui, state: &mut PaletteState| {
            orchestrator_studio::shell::show_top_bar(ui, &mut state.ctx, &mut state.open);
            if state.open {
                let entries = items.clone();
                if let Some(picked) = orchestrator_studio::shell::show_palette(
                    ui,
                    &mut state.open,
                    &mut state.query,
                    &entries,
                ) {
                    state.picked.push(picked.title);
                }
            }
        },
        PaletteState {
            ctx: ShellContext::default(),
            open: true,
            query: String::new(),
            picked: Vec::new(),
        },
    );
    harness.run();
    harness.key_press(egui::Key::Escape);
    harness.run();
    assert!(!harness.state().open);
    assert!(harness.state().picked.is_empty());
}
