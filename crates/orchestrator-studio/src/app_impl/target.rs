//! Parte de `app_impl` (extraída para manter os arquivos revisáveis).

use super::super::*;

impl StudioApp {
    pub(super) fn sync_target(&mut self, ui: &egui::Ui) {
        // T-890-03: descoberta → alvo único. O nó selecionado (auto: primeiro
        // online; manual: combobox no painel Máquinas) propaga para TODOS os
        // painéis que falam com um studio-node — trocar uma vez, muda tudo.
        self.discovery.poll();
        // `STUDIO_TARGET_URL` (capturas de verificação): força o alvo para a
        // URL dada assim que ela aparece na descoberta — determinístico. Sem
        // a env, valem auto-seleção/seleção manual normais.
        if let Ok(wanted) = std::env::var("STUDIO_TARGET_URL") {
            let wanted = wanted.trim().to_string();
            if !wanted.is_empty()
                && self.discovery.selected_url().as_deref() != Some(wanted.as_str())
            {
                if let Some(index) = self
                    .discovery
                    .nodes
                    .iter()
                    .position(|node| node.url == wanted)
                {
                    self.discovery.select(index);
                    orchestrator_studio::studio_log::info(format!("alvo forçado (env): {wanted}"));
                }
            }
        }
        // Drena a leitura do nó alvo em QUALQUER tela (o header e a barra de
        // status dependem dela — antes só a view Nó drenava).
        self.state.poll();
        if self.discovery.autoselect_first_online() {
            orchestrator_studio::studio_log::info(format!(
                "alvo automático: {}",
                self.discovery.selected_url().unwrap_or_default()
            ));
        }
        if let Some(target) = self.discovery.selected_url() {
            self.node_url = target.clone();
            self.services.url = target.clone();
            self.shared.url = target.clone();
            self.machines.url = target.clone();
            // AUTO-CARGA (T-890-03): trocou o alvo → os painéis disparam as
            // próprias leituras SOZINHOS (Nó conecta, Serviços lê o plano,
            // Catálogo tira o snapshot). Token do deploy quando existir.
            if self.autoloaded_target.as_deref() != Some(target.as_str()) {
                self.autoloaded_target = Some(target.clone());
                let token = orchestrator_studio::discovery::token_for_url(
                    &target,
                    std::env::var("HOME").ok().as_deref(),
                );
                if let Some(token) = &token {
                    self.node_token = token.clone();
                }
                orchestrator_studio::studio_log::info(format!(
                    "auto-carga do alvo {target}: Nó (token={}) + Serviços + Catálogo",
                    if token.is_some() { "sim" } else { "não" }
                ));
                self.state
                    .refresh_from_node_with_token(&target, token.as_deref());
                self.services.refresh();
                self.shared.refresh();
                self.machines.refresh();
            }
        }

        // Auto-carga contínua (toggle do header, mockup "Auto-Carga (5s)"):
        // re-leituras periódicas do alvo — cada refresh é no-op enquanto busy.
        if self.auto_reload {
            ui.ctx()
                .request_repaint_after(std::time::Duration::from_secs(1));
            if self
                .last_autoload
                .is_none_or(|at| at.elapsed().as_secs() >= 5)
            {
                self.last_autoload = Some(std::time::Instant::now());
                if let Some(target) = self.discovery.selected_url() {
                    let token = orchestrator_studio::discovery::token_for_url(
                        &target,
                        std::env::var("HOME").ok().as_deref(),
                    );
                    self.state
                        .refresh_from_node_with_token(&target, token.as_deref());
                    self.services.refresh();
                    self.shared.refresh();
                    self.machines.refresh();
                }
            }
        }

        // Topologia/Ferramentas: primeira observação automática ao entrar na
        // aba (domínio default já é o da descoberta — ver DdsState::new).
        #[cfg(feature = "dds")]
        if matches!(self.section, Section::Topology | Section::Tools)
            && !self.dds_observed
            && !self.dds.busy
        {
            self.dds_observed = true;
            orchestrator_studio::studio_log::info(format!(
                "topologia: observação automática do domínio {}",
                self.dds.domain
            ));
            self.dds.refresh();
        }
    }
}
