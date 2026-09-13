//! Painel SSH dedicado: identidade, desbloqueio, aprovação e execução.
//!
//! Chave dedicada por instalação (gerada aqui, privada cifrada 0600, senha
//! só em memória). Conexão e desbloqueio rodam em thread dedicada — a UI
//! nunca trava. Sem senha SSH, sem agente, sem shell.

use eframe::egui;
use orchestrator_studio::nodes::NodeRegistry;
use orchestrator_studio::ssh_session::{SshPhase, SshSession};

/// Aplica o diretório informado no painel: identidade e cofre ficam
/// juntos no mesmo diretório. Regressão G-04: `trust_path` precisa
/// acompanhar `identity_pem` — sem isso a aprovação falhava com
/// "arquivo de confiança ilegível em :" (caminho vazio).
pub fn aplicar_diretorio(session: &mut SshSession, dir: &str) {
    session.config.identity_pem = std::path::PathBuf::from(dir).join("studio_ed25519");
    session.config.trust_path = std::path::PathBuf::from(dir).join("trust.json");
}

pub fn show(ui: &mut egui::Ui, session: &mut SshSession, registry: &NodeRegistry) {
    session.set_repaint_source(ui.ctx().clone());
    session.poll();
    ui.heading("SSH dedicado (bridge integrada, G-04)");
    if matches!(session.phase, SshPhase::Running) {
        ui.ctx().request_repaint();
    }
    ui.label("Identidade Ed25519 desta instalação; só a pública viaja no provisionamento.");
    ui.horizontal(|ui| {
        ui.label("diretório:");
        let mut dir = session
            .config
            .identity_pem
            .parent()
            .map(|parent| parent.display().to_string())
            .unwrap_or_default();
        if ui.text_edit_singleline(&mut dir).changed() {
            aplicar_diretorio(session, &dir);
        }
        if ui.button("Gerar identidade").clicked() {
            let dir = std::path::PathBuf::from(&dir);
            match studio_ssh::generate(&dir, &session.config.passphrase) {
                Ok(paths) => {
                    session.config.identity_pem = paths.private_pem;
                    session.phase = SshPhase::Idle;
                }
                Err(err) => {
                    session.phase = SshPhase::Failed {
                        error: err.to_string(),
                    };
                }
            }
        }
    });
    ui.horizontal(|ui| {
        ui.label("senha de desbloqueio:");
        ui.add(egui::TextEdit::singleline(&mut session.config.passphrase).password(true));
        ui.label("(só em memória; gerar exige senha não vazia)");
    });
    ui.separator();
    ui.label("Alvo (use o registro de Nós; digite usuário e comando):");
    ui.horizontal(|ui| {
        ui.label("host:");
        ui.text_edit_singleline(&mut session.config.target.host);
        let mut port = session.config.target.port.to_string();
        ui.label("porta:");
        if ui.text_edit_singleline(&mut port).changed() {
            session.config.target.port = port.parse().unwrap_or(session.config.target.port);
        }
        ui.label("usuário:");
        ui.text_edit_singleline(&mut session.config.target.username);
    });
    ui.horizontal(|ui| {
        ui.label("comando:");
        ui.text_edit_singleline(&mut session.config.command);
        if ui.button("Conectar e executar").clicked() {
            let selected = registry
                .selected()
                .map(|entry| (entry.alias.clone(), entry.url.clone()));
            if let Some((alias, _)) = selected {
                session.config.alias = alias;
            }
            session.start();
        }
        if matches!(session.phase, SshPhase::Running) {
            ui.spinner();
            ui.label("executando… a UI segue livre");
        }
    });
    let phase = session.phase.clone();
    match phase {
        SshPhase::Idle | SshPhase::Running => {}
        SshPhase::NeedsApproval {
            key_type,
            fingerprint,
        } => {
            ui.group(|ui| {
                ui.strong("Host desconhecido — confira por fonte independente antes de aprovar:");
                ui.monospace(format!("{key_type} {fingerprint}"));
                if ui.button("Aprovar e salvar no cofre").clicked() {
                    if let Err(err) = session.approve_displayed("operador-gui") {
                        session.phase = SshPhase::Failed { error: err };
                    }
                }
            });
        }
        SshPhase::Done { output } => {
            ui.group(|ui| {
                ui.strong("Saída:");
                ui.monospace(output);
            });
        }
        SshPhase::Failed { error } => {
            ui.label(format!("Falhou (sem fallback): {error}"));
        }
    }
    ui.label("Cofre: aprovações por host:porta com contexto; chave alterada bloqueia.");
}

#[cfg(test)]
mod tests {
    use super::*;

    /// Regressão G-04: informar o diretório no painel precisa configurar
    /// identidade E cofre. O bug antigo deixava `trust_path` vazio e a
    /// aprovação falhava com "arquivo de confiança ilegível em :".
    #[test]
    fn aplicar_diretorio_configura_identidade_e_cofre() {
        let mut session = SshSession::new();
        assert!(session.config.trust_path.as_os_str().is_empty());

        aplicar_diretorio(&mut session, "/tmp/studio-gui-regressao");

        assert_eq!(
            session.config.identity_pem,
            std::path::PathBuf::from("/tmp/studio-gui-regressao/studio_ed25519")
        );
        assert_eq!(
            session.config.trust_path,
            std::path::PathBuf::from("/tmp/studio-gui-regressao/trust.json")
        );
    }

    /// O campo usuário do alvo começa vazio e é preenchido explicitamente
    /// pelo operador — nada de usuário implícito.
    #[test]
    fn usuario_do_alvo_comeca_vazio() {
        let session = SshSession::new();
        assert!(session.config.target.username.is_empty());
    }
}
