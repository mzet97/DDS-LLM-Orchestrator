//! Seletores dependentes (UI-3.4): mudar o host invalida dispositivo
//! e caminho com aviso localizado; campos independentes (nome,
//! instruções) são preservados. Sem `GPU 0` silencioso de outra máquina.

/// Seleção de destino com dependências por host.
#[derive(Debug, Clone, PartialEq, Eq, Default)]
pub struct DestinationPick {
    pub host: String,
    pub device: Option<String>,
    pub artifact_path: Option<String>,
    pub notice: String,
}

impl DestinationPick {
    #[must_use]
    pub fn new() -> Self {
        Self::default()
    }

    /// Troca o host: dispositivo e caminho passam a exigir nova escolha,
    /// com aviso localizado. Não toca em mais nada.
    pub fn change_host(&mut self, host: &str) {
        if self.host == host {
            return;
        }
        self.host = String::from(host);
        let had_dependents = self.device.is_some() || self.artifact_path.is_some();
        self.device = None;
        self.artifact_path = None;
        self.notice = if had_dependents {
            String::from("Host alterado: escolha novamente dispositivo e caminho do artefato.")
        } else {
            String::new()
        };
    }

    /// Rótulo de caminho sempre com a máquina (T07: "Arquivo em X").
    #[must_use]
    pub fn path_label(&self) -> String {
        match &self.artifact_path {
            Some(path) => format!("Arquivo em {}: {path}", self.host),
            None => String::from("nenhum artefato escolhido"),
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn host_change_invalidates_dependents_with_notice() {
        let mut pick = DestinationPick {
            host: String::from("gpu-a"),
            device: Some(String::from("GPU 0")),
            artifact_path: Some(String::from("/m/modelo.gguf")),
            notice: String::new(),
        };
        pick.change_host("gpu-b");
        assert_eq!(pick.device, None);
        assert_eq!(pick.artifact_path, None);
        assert!(pick.notice.contains("escolha novamente"));
    }

    #[test]
    fn same_host_keeps_everything() {
        let mut pick = DestinationPick {
            host: String::from("gpu-a"),
            device: Some(String::from("GPU 0")),
            artifact_path: None,
            notice: String::new(),
        };
        pick.change_host("gpu-a");
        assert_eq!(pick.device.as_deref(), Some("GPU 0"));
        assert!(pick.notice.is_empty());
    }

    #[test]
    fn path_label_always_names_machine() {
        let mut pick = DestinationPick::new();
        pick.host = String::from("gpu-amd-01");
        pick.artifact_path = Some(String::from("/m/a.gguf"));
        assert_eq!(pick.path_label(), "Arquivo em gpu-amd-01: /m/a.gguf");
    }
}
