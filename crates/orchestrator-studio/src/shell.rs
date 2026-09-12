//! Estado do shell da aplicação (UI-1.2/UI-1.4): contexto visível,
//! painel de operações e paleta de comandos. Lógica pura; a casca
//! eframe apenas renderiza.

use crate::design::Theme;

/// Contexto sempre visível na barra superior (§5: projeto, domínio,
/// fonte dos dados).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct ShellContext {
    pub project: String,
    pub domain: String,
    pub source: DataSource,
    pub theme: Theme,
}

/// Origem dos dados em exibição (dimensão "Origem", §13).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DataSource {
    Live,
    Cache,
    Imported,
    Demo,
}

impl DataSource {
    /// Rótulo curto em pt-BR para a barra superior.
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Live => "ao vivo",
            Self::Cache => "cache local",
            Self::Imported => "registro importado",
            Self::Demo => "Demonstração · dados simulados",
        }
    }
}

impl Default for ShellContext {
    fn default() -> Self {
        Self {
            project: String::from("Laboratório"),
            domain: String::from("78"),
            source: DataSource::Live,
            theme: Theme::default(),
        }
    }
}

/// Item pesquisável da paleta de comandos (§6: pesquisa ações, não
/// executa perigosa por Enter ambíguo).
#[derive(Debug, Clone, PartialEq, Eq)]
pub struct CommandItem {
    pub title: String,
    pub section: String,
    pub dangerous: bool,
}

impl CommandItem {
    #[must_use]
    pub fn matches(&self, query: &str) -> bool {
        let query = query.to_lowercase();
        self.title.to_lowercase().contains(&query) || self.section.to_lowercase().contains(&query)
    }
}

/// Filtra itens por consulta; vazia retorna todos. Itens perigosos nunca
/// são o primeiro resultado de consulta ambígua: vão para o fim.
#[must_use]
pub fn filter_commands<'a>(items: &'a [CommandItem], query: &str) -> Vec<&'a CommandItem> {
    let mut matched: Vec<&CommandItem> = items.iter().filter(|item| item.matches(query)).collect();
    matched.sort_by_key(|item| item.dangerous);
    matched
}

/// Enter ativa o primeiro resultado NÃO perigoso (ou nada, se o primeiro
/// for perigoso). Perigoso exige clique explícito — e mesmo assim o
/// chamador deve pedir revisão antes de qualquer efeito.
#[must_use]
pub fn enter_picks<'a>(ranked: &[&'a CommandItem]) -> Option<&'a CommandItem> {
    match ranked.first() {
        Some(item) if !item.dangerous => Some(*item),
        _ => None,
    }
}

/// Barra superior: projeto, domínio, fonte dos dados, tema e paleta.
pub fn show_top_bar(ui: &mut egui::Ui, ctx: &mut ShellContext, palette_open: &mut bool) {
    ui.horizontal(|ui| {
        ui.strong(format!("Projeto: {}", ctx.project));
        ui.label(format!("Domínio: {}", ctx.domain));
        ui.label(format!("Fonte: {}", ctx.source.label()));
        ui.separator();
        egui::ComboBox::from_label("Tema")
            .selected_text(ctx.theme.label())
            .show_ui(ui, |ui| {
                ui.selectable_value(&mut ctx.theme, Theme::System, Theme::System.label());
                ui.selectable_value(&mut ctx.theme, Theme::Light, Theme::Light.label());
                ui.selectable_value(&mut ctx.theme, Theme::Dark, Theme::Dark.label());
            });
        if ui.button("Buscar / Ctrl+K").clicked() {
            *palette_open = true;
        }
    });
}

/// Janela da paleta de comandos. Retorna o item escolhido por clique ou
/// Enter (Enter nunca ativa perigoso, ver [`enter_picks`]).
pub fn show_palette(
    ui: &mut egui::Ui,
    open: &mut bool,
    query: &mut String,
    items: &[CommandItem],
) -> Option<CommandItem> {
    let mut picked: Option<CommandItem> = None;
    let mut escape = false;
    egui::Window::new("Paleta de comandos")
        .open(open)
        .show(ui.ctx(), |ui| {
            if ui.input(|input| input.key_pressed(egui::Key::Escape)) {
                escape = true;
            }
            let response = ui.text_edit_singleline(query);
            response.request_focus();
            let ranked = filter_commands(items, query);
            if ui.input(|input| input.key_pressed(egui::Key::Enter)) {
                picked = enter_picks(&ranked).cloned();
            }
            egui::ScrollArea::vertical().show(ui, |ui| {
                for item in ranked {
                    let label = if item.dangerous {
                        format!("{} — {} (requer revisão)", item.section, item.title)
                    } else {
                        format!("{} — {}", item.section, item.title)
                    };
                    if ui.selectable_label(false, label).clicked() {
                        picked = Some(item.clone());
                    }
                }
            });
        });
    if picked.is_some() || escape {
        *open = false;
        query.clear();
    }
    picked
}

#[cfg(test)]
mod tests {
    use super::*;

    fn catalog() -> Vec<CommandItem> {
        vec![
            CommandItem {
                title: String::from("Ir para Agentes"),
                section: String::from("Recursos"),
                dangerous: false,
            },
            CommandItem {
                title: String::from("Remover do projeto"),
                section: String::from("Recursos"),
                dangerous: true,
            },
        ]
    }

    #[test]
    fn dangerous_item_never_first_on_ambiguous_query() {
        let items = catalog();
        let results = filter_commands(&items, "re");
        assert_eq!(results.len(), 2);
        assert!(!results[0].dangerous);
        assert!(results[1].dangerous);
    }

    #[test]
    fn empty_query_returns_all_safe_first() {
        let items = catalog();
        let results = filter_commands(&items, "");
        assert_eq!(results.len(), 2);
        assert!(!results[0].dangerous);
    }

    #[test]
    fn enter_never_picks_dangerous_first() {
        let items = catalog();
        let ranked = filter_commands(&items, "");
        assert_eq!(
            enter_picks(&ranked).map(|item| &item.title),
            Some(&String::from("Ir para Agentes"))
        );
        let only_dangerous = vec![CommandItem {
            title: String::from("Remover do projeto"),
            section: String::from("Recursos"),
            dangerous: true,
        }];
        let ranked = filter_commands(&only_dangerous, "");
        assert_eq!(enter_picks(&ranked), None);
    }

    #[test]
    fn demo_source_label_identifies_simulation() {
        assert_eq!(DataSource::Demo.label(), "Demonstração · dados simulados");
    }
}
