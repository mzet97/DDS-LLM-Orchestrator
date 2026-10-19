//! Painel SSH Dedicado (T-800-29, G-04): formulário de alvo +
//! identidade dedicada + cofre de confiança sobre a bridge russh.
//!
//! Só fatos reais: a impressão exibida para aprovação é a do handshake
//! (erro tipado da bridge), a saída é a do comando remoto e a lista de
//! aprovações é lida do cofre JSON a cada frame. Senha só em memória.

use crate::kit;
use crate::panel_header::panel_header;
use crate::ssh_session::{SshPhase, SshSession};
use crate::theme;
use eframe::egui;

fn field(ui: &mut egui::Ui, label: &str, value: &mut String, hint: &str) {
    ui.label(
        egui::RichText::new(label)
            .monospace()
            .small()
            .color(theme::OUTLINE),
    );
    ui.add(egui::TextEdit::singleline(value).hint_text(hint));
    ui.add_space(theme::SPACE_XS);
}

fn phase_badge(ui: &mut egui::Ui, session: &SshSession) {
    let (text, color) = match &session.phase {
        SshPhase::Idle => ("OCIOSA", theme::STALE),
        SshPhase::Connecting => ("CONECTANDO…", theme::PRIMARY_FIXED_DIM),
        SshPhase::AwaitingApproval { .. } => ("AGUARDANDO APROVAÇÃO", theme::AUTH),
        SshPhase::Done => ("CONCLUÍDA", theme::OK),
        SshPhase::Failed => ("FALHOU", theme::ERROR),
    };
    kit::badge(ui, text, color);
}

pub fn show(ui: &mut egui::Ui, session: &mut SshSession) {
    session.poll();
    if session.busy() {
        ui.ctx()
            .request_repaint_after(std::time::Duration::from_millis(100));
    }
    panel_header(
        ui,
        "SEC 3.15 · SSH DEDICADO — BRIDGE RUSSH (G-04)",
        "SSH Dedicado",
        "Chave Ed25519 dedicada por instalação · host key conferida no \
         handshake · sem senha de login, sem agente, sem fallback",
    );
    ui.add_space(theme::SPACE_SM);
    ui.horizontal(|ui| {
        phase_badge(ui, session);
        if session.identity_exists() {
            kit::badge(ui, "IDENTIDADE OK", theme::OK);
        } else {
            kit::badge(ui, "SEM IDENTIDADE", theme::ERROR);
        }
        ui.label(
            egui::RichText::new(format!(
                "identidade: {} · cofre: {}",
                session.private_pem().display(),
                session.trust_path.display()
            ))
            .monospace()
            .small()
            .color(theme::OUTLINE),
        );
    });
    ui.add_space(theme::SPACE_SM);

    ui.columns(2, |cols| {
        cols[0].vertical(|ui| {
            kit::section_label(ui, "ALVO E COMANDO");
            field(ui, "HOST", &mut session.host, "192.168.1.61");
            field(ui, "PORTA", &mut session.port, "22");
            field(ui, "USUÁRIO", &mut session.username, "tese");
            field(ui, "APELIDO (cofre)", &mut session.alias, "gpu-61");
            field(ui, "PROJETO (cofre)", &mut session.project, "studio");
            field(ui, "OPERADOR (cofre)", &mut session.operator, "seu nome");
            ui.label(
                egui::RichText::new("SENHA DA IDENTIDADE (só memória)")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.add(
                egui::TextEdit::singleline(&mut session.passphrase)
                    .password(true)
                    .hint_text("nunca gravada"),
            );
            ui.add_space(theme::SPACE_XS);
            ui.label(
                egui::RichText::new("COMANDO REMOTO")
                    .monospace()
                    .small()
                    .color(theme::OUTLINE),
            );
            ui.add(egui::TextEdit::singleline(&mut session.command).desired_width(f32::INFINITY));
            ui.add_space(theme::SPACE_SM);
            ui.horizontal(|ui| {
                ui.add_enabled_ui(!session.busy(), |ui| {
                    if kit::primary_button(ui, "Executar via SSH").clicked() {
                        session.start();
                    }
                    if ui.button("Gerar identidade").clicked() {
                        session.generate_identity();
                    }
                });
            });
        });
        cols[1].vertical(|ui| {
            kit::section_label(ui, "CONFIANÇA E SAÍDA");
            if let SshPhase::AwaitingApproval {
                key_type,
                fingerprint,
            } = &session.phase
            {
                kit::warn_banner(
                    ui,
                    &format!(
                        "Host desconhecido: confira {fingerprint} ({key_type}) por fonte \
                         independente antes de aprovar."
                    ),
                );
                ui.add_space(theme::SPACE_XS);
                if kit::primary_button(ui, "Aprovar impressão e reconectar").clicked() {
                    session.approve_displayed();
                }
                ui.add_space(theme::SPACE_SM);
            }
            if !session.notice.is_empty() {
                ui.label(
                    egui::RichText::new(&session.notice)
                        .monospace()
                        .small()
                        .color(theme::ON_SURFACE_VARIANT),
                );
                ui.add_space(theme::SPACE_XS);
            }
            kit::section_label(ui, "SAÍDA DO COMANDO");
            egui::ScrollArea::vertical()
                .max_height(180.0)
                .show(ui, |ui| {
                    ui.label(
                        egui::RichText::new(if session.output.is_empty() {
                            "—"
                        } else {
                            &session.output
                        })
                        .monospace()
                        .small(),
                    );
                });
            ui.add_space(theme::SPACE_SM);
            kit::section_label(ui, "APROVAÇÕES NO COFRE");
            let approvals = session.approvals();
            if approvals.is_empty() {
                kit::empty_state(ui, "Nenhuma aprovação persistida.");
            } else {
                for approval in &approvals {
                    ui.label(
                        egui::RichText::new(format!(
                            "{} ({}:{}) · {} · por {}",
                            approval.alias,
                            approval.host,
                            approval.port,
                            approval.key.fingerprint,
                            approval.approved_by
                        ))
                        .monospace()
                        .small(),
                    );
                }
            }
        });
    });
}
