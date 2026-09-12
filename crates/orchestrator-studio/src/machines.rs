//! Máquinas e rede (T03/UI-2.2): identidade, saúde, confiança,
//! administração e evidência DDS em campos DISTINTOS. A tela nunca
//! implementa scanner; ações só emitem intenções.

use serde::{Deserialize, Serialize};

/// Conectividade administrativa (saúde do canal de gestão).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum AdminState {
    Managed,
    Unreachable,
    Unknown,
}

impl AdminState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Managed => "gerenciada",
            Self::Unreachable => "inacessível",
            Self::Unknown => "não verificado",
        }
    }
}

/// Confiança na identidade (campo separado da saúde).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum TrustState {
    Verified,
    Unverified,
    Divergent,
}

impl TrustState {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Verified => "verificada",
            Self::Unverified => "não verificada",
            Self::Divergent => "identidade divergente",
        }
    }
}

/// Evidência DDS disponível (não é prova de administração).
#[derive(Debug, Clone, Copy, PartialEq, Eq, Serialize, Deserialize)]
pub enum DdsEvidence {
    Observed,
    Partial,
    None,
}

impl DdsEvidence {
    #[must_use]
    pub const fn label(self) -> &'static str {
        match self {
            Self::Observed => "observada",
            Self::Partial => "parcial",
            Self::None => "ausente",
        }
    }
}

/// Máquina conhecida do projeto.
#[derive(Debug, Clone, PartialEq, Serialize, Deserialize)]
pub struct MachineEntry {
    pub alias: String,
    pub endpoint: String,
    pub admin: AdminState,
    pub trust: TrustState,
    pub dds: DdsEvidence,
    pub services: Vec<String>,
    pub origin: String,
    pub last_seen: String,
}

/// Registro de máquinas digitadas pelo operador (sem varredura).
#[derive(Debug, Clone, Default)]
pub struct MachineLedger {
    entries: Vec<MachineEntry>,
    selected: Option<String>,
    pub draft_alias: String,
    pub draft_endpoint: String,
    pub error: String,
}

impl MachineLedger {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Adiciona máquina; apelido vazio ou duplicado é erro tipado.
    pub fn add(&mut self, alias: &str, endpoint: &str) -> Result<(), String> {
        let alias = alias.trim();
        if alias.is_empty() {
            return Err(String::from("apelido vazio"));
        }
        if self.entries.iter().any(|entry| entry.alias == alias) {
            return Err(format!("máquina duplicada: {alias}"));
        }
        self.entries.push(MachineEntry {
            alias: String::from(alias),
            endpoint: String::from(endpoint.trim()),
            admin: AdminState::Unknown,
            trust: TrustState::Unverified,
            dds: DdsEvidence::None,
            services: Vec::new(),
            origin: String::from("cadastro local"),
            last_seen: String::from("nunca observado"),
        });
        Ok(())
    }

    /// Remove por apelido; seleção removida volta a nada.
    pub fn remove(&mut self, alias: &str) {
        self.entries.retain(|entry| entry.alias != alias);
        if self.selected.as_deref() == Some(alias) {
            self.selected = None;
        }
    }

    /// Seleciona por apelido; inexistente limpa a seleção.
    pub fn select(&mut self, alias: &str) {
        if self.entries.iter().any(|entry| entry.alias == alias) {
            self.selected = Some(String::from(alias));
        } else {
            self.selected = None;
        }
    }

    #[must_use]
    pub fn list(&self) -> &[MachineEntry] {
        &self.entries
    }

    #[must_use]
    pub fn selected(&self) -> Option<&MachineEntry> {
        let alias = self.selected.as_ref()?;
        self.entries.iter().find(|entry| &entry.alias == alias)
    }
}

/// Tabela selecionável por apelido (ID estável). O chamador aplica a
/// seleção; aqui só renderização + intenção de clique.
pub fn show_table(ui: &mut egui::Ui, ledger: &MachineLedger, on_select: &mut dyn FnMut(&str)) {
    egui::Grid::new("machines_grid").show(ui, |ui| {
        ui.label("máquina");
        ui.label("administração");
        ui.label("confiança");
        ui.label("evidência DDS");
        ui.label("origem");
        ui.end_row();
        for entry in ledger.list() {
            let selected =
                ledger.selected().map(|item| item.alias.as_str()) == Some(entry.alias.as_str());
            if ui.selectable_label(selected, &entry.alias).clicked() {
                on_select(&entry.alias);
            }
            ui.label(entry.admin.label());
            ui.label(entry.trust.label());
            ui.label(entry.dds.label());
            ui.label(&entry.origin);
            ui.end_row();
        }
    });
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn add_select_remove_roundtrip() {
        let mut ledger = MachineLedger::new();
        assert!(ledger.add("gpu-amd-01", "http://192.168.1.61:4317").is_ok());
        ledger.select("gpu-amd-01");
        assert_eq!(
            ledger.selected().map(|entry| entry.alias.as_str()),
            Some("gpu-amd-01")
        );
        ledger.remove("gpu-amd-01");
        assert!(ledger.list().is_empty());
        assert_eq!(ledger.selected(), None);
    }

    #[test]
    fn duplicate_and_empty_alias_rejected() {
        let mut ledger = MachineLedger::new();
        assert!(ledger.add("n1", "http://x").is_ok());
        assert!(ledger.add("n1", "http://y").is_err());
        assert!(ledger.add("  ", "http://y").is_err());
        assert_eq!(ledger.list().len(), 1);
    }

    #[test]
    fn defaults_keep_dimensions_distinct() {
        let mut ledger = MachineLedger::new();
        ledger.add("n1", "http://x").expect("adiciona");
        let entry = &ledger.list()[0];
        assert_eq!(entry.admin, AdminState::Unknown);
        assert_eq!(entry.trust, TrustState::Unverified);
        assert_eq!(entry.dds, DdsEvidence::None);
    }
}
