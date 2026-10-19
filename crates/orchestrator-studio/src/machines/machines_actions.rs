//! Parte de `machines` (extraída para manter os arquivos revisáveis).

use super::*;

impl MachinesState {
    /// Sonda a máquina (`GET {node_url}/version`, T-840-03c) em background,
    /// com o token em memória para aquele `node_url` — se houver. Mapa vazio
    /// no arranque → nós protegidos respondem 401 → dica "token ausente" com
    /// o formulário para (re)entrar. Clique durante `busy` é ignorado.
    pub fn probe(&mut self, machine: &MachineRecord) {
        self.probe_url(&machine.node_url);
        if self.busy {
            self.notice = format!("sondando {}…", machine.machine_id);
        }
    }

    /// Sonda uma URL descoberta (mesma sonda do catálogo; usada pelo botão
    /// "Forçar probe"/"Re-testar" da tabela 3.10). Clique durante `busy` é
    /// ignorado.
    pub fn probe_url(&mut self, node_url: &str) {
        if self.busy {
            return;
        }
        let url = normalize_url(node_url);
        let token = self.tokens.get(&url).cloned();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let status = probe_status(&url, token.as_deref());
            let _ = tx.send(MachinesMsg::Probed {
                node_url: url,
                status,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.pending_probes = 1;
        self.notice = String::from("sondando…");
    }

    /// Sonda TODAS as URLs descobertas (botão "Forçar Probe" da 3.10): uma
    /// thread por URL no mesmo canal; `poll` drena e só libera `busy` quando
    /// a última responder. Clique durante `busy` ou lista vazia é ignorado.
    pub fn probe_all(&mut self, node_urls: &[String]) {
        if self.busy || node_urls.is_empty() {
            return;
        }
        let (tx, rx) = mpsc::channel();
        for node_url in node_urls {
            let url = normalize_url(node_url);
            let token = self.tokens.get(&url).cloned();
            let tx = tx.clone();
            std::thread::spawn(move || {
                let status = probe_status(&url, token.as_deref());
                let _ = tx.send(MachinesMsg::Probed {
                    node_url: url,
                    status,
                });
            });
        }
        self.receiver = Some(rx);
        self.busy = true;
        self.pending_probes = node_urls.len();
        self.notice = format!("sondando {} nó(s)…", node_urls.len());
    }

    /// "Testar & Registrar" (design 3.10): valida o formulário com um probe
    /// imediato e SÓ publica no catálogo se voltar `Online`. Exige
    /// `machine_id` + `node_url` (mesma regra do publicar direto).
    pub fn test_and_register(&mut self) {
        if self.busy {
            return;
        }
        let node_url = normalize_url(&self.form_node_url);
        if self.form_machine_id.trim().is_empty() || node_url.is_empty() {
            self.notice = String::from("machine_id e node_url são obrigatórios");
            return;
        }
        if let Some(token) = optional_token(&self.form_token) {
            self.tokens.insert(node_url.clone(), String::from(token));
        }
        self.pending_publish_after_probe = true;
        self.probe_url(&node_url);
        self.notice = format!("validando {node_url} antes de registrar…");
    }

    /// Limpa o formulário de registro manual (botão "Limpar" da 3.10).
    pub fn clear_form(&mut self) {
        self.form_machine_id.clear();
        self.form_node_url.clear();
        self.form_token.clear();
    }

    /// Guarda token de uma URL descoberta na memória local (modal 3.10
    /// "Definir como Alvo & Configurar Token"). NUNCA vai ao catálogo nem a
    /// disco (RNF-04).
    pub fn remember_token(&mut self, node_url: &str, token: &str) {
        let url = normalize_url(node_url);
        let trimmed = token.trim();
        if trimmed.is_empty() {
            self.tokens.remove(&url);
        } else {
            self.tokens.insert(url, String::from(trimmed));
        }
    }

    /// Drena o worker; chamar a cada frame enquanto `busy` (padrão
    /// `catalog_remote::poll` — nota idêntica: dentro de `poll` o snapshot é
    /// aplicado por atribuição direta de campos).
    pub fn poll(&mut self) {
        let mut finished = false;
        // Drena para um Vec antes de processar: os braços podem rearmar
        // `busy`/`receiver` (encadeamento do "Testar & Registrar") sem
        // conflitar com o borrow do canal.
        let pending: Vec<MachinesMsg> = self
            .receiver
            .as_ref()
            .map(|rx| rx.try_iter().collect())
            .unwrap_or_default();
        for msg in pending {
            match msg {
                MachinesMsg::Snapshot(result) => {
                    match result {
                        Ok(snapshot) => {
                            self.cursor = snapshot.cursor.0;
                            self.snapshot = Some(snapshot);
                            self.notice.clear();
                        }
                        Err(err) => {
                            self.notice = err;
                        }
                    }
                    finished = true;
                }
                MachinesMsg::Mutated {
                    notice,
                    fill_base,
                    then_snapshot,
                } => {
                    if let Some(current) = fill_base {
                        // 409: base do formulário preenchida com a vigente
                        // para o próximo clique (T-840-03c).
                        self.form_base = current.to_string();
                        self.notice = format!("{notice}; base preenchida com a vigente");
                    } else {
                        self.notice = notice;
                    }
                    if let Some(result) = then_snapshot {
                        match result {
                            Ok(snapshot) => {
                                self.cursor = snapshot.cursor.0;
                                self.snapshot = Some(snapshot);
                            }
                            Err(err) => {
                                self.notice = err;
                            }
                        }
                    }
                    finished = true;
                }
                MachinesMsg::Probed { node_url, status } => {
                    let online = status.state == ProbeState::Online;
                    let detail = status.detail.clone();
                    self.notice = detail.clone();
                    self.probes.insert(node_url.clone(), status);
                    self.pending_probes = self.pending_probes.saturating_sub(1);
                    if self.pending_publish_after_probe {
                        // "Testar & Registrar": Online publica, resto cancela.
                        self.pending_publish_after_probe = false;
                        if online {
                            self.busy = false;
                            self.receiver = None;
                            let _ = self.publish_machine();
                        } else {
                            self.notice = format!(
                                "probe de {node_url} não passou ({detail}); registro NÃO \
                                     persistido"
                            );
                            if self.pending_probes == 0 {
                                finished = true;
                            }
                        }
                    } else if self.pending_probes == 0 {
                        finished = true;
                    }
                }
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
        }
    }
}
