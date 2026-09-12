//! Painel SSH dedicado: identidade, desbloqueio, aprovação e execução.
//!
//! Chave dedicada por instalação (gerada aqui, privada cifrada 0600, senha
//! só em memória). Conexão e desbloqueio rodam em thread dedicada — a UI
//! nunca trava. Sem senha SSH, sem agente, sem shell.

use eframe::egui;
use orchestrator_studio::nodes::NodeRegistry;
use orchestrator_studio::ssh_session::{SshPhase, SshSession};

pub fn show(ui: &mut egui::Ui, session: &mut SshSession, registry: &NodeRegistry) {
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
            session.config.identity_pem = std::path::PathBuf::from(&dir).join("studio_ed25519");
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
