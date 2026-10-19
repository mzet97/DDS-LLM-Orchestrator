//! Parte de `machines` (extraída para manter os arquivos revisáveis).

use super::*;

impl MachinesState {
    /// Padrão honesto: autoridade local, sem snapshot nem tokens.
    #[must_use]
    pub fn with_url(url: &str) -> Self {
        Self {
            url: String::from(url),
            ..Self::default()
        }
    }

    /// Token efetivo da autoridade: campo explícito, ou o token em memória
    /// guardado para a URL da autoridade (modal/formulário da 3.10), ou o
    /// token de deploy em disco (mesma fonte do painel Nó/auto-carga) — a
    /// autoridade é o alvo global (`main` sincroniza `url` a cada frame).
    pub(super) fn auth(&self) -> Option<String> {
        if let Some(token) = optional_token(&self.authority_token) {
            return Some(token.to_string());
        }
        let normalized = normalize_url(&self.url);
        if let Some(token) = self.tokens.get(&normalized) {
            return Some(token.clone());
        }
        crate::discovery::token_for_url(&self.url, std::env::var("HOME").ok().as_deref())
    }

    /// Token em memória da máquina (chave normalizada) — nunca sai da GUI.
    #[must_use]
    pub fn token_for(&self, node_url: &str) -> Option<&str> {
        self.tokens
            .get(&normalize_url(node_url))
            .map(String::as_str)
    }

    /// URLs sondadas com `AuthPending` e SEM token em memória — a dica
    /// "token ausente" do painel (T-840-03c).
    #[must_use]
    pub fn missing_token_urls(&self) -> Vec<String> {
        self.probes
            .iter()
            .filter(|(url, status)| {
                status.state == ProbeState::AuthPending && !self.tokens.contains_key(*url)
            })
            .map(|(url, _)| url.clone())
            .collect()
    }

    /// Máquinas do snapshot corrente, ordenadas (só `kind=="machine"`).
    #[must_use]
    pub fn machines(&self) -> Vec<MachineRecord> {
        self.snapshot
            .as_ref()
            .map(machines_from_snapshot)
            .unwrap_or_default()
    }

    /// Relê o snapshot da autoridade em background. Clique durante `busy`
    /// é ignorado (mesma disciplina do `catalog_remote`).
    pub fn refresh(&mut self) {
        if self.busy {
            return;
        }
        let url = self.url.clone();
        let auth = self.auth();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(MachinesMsg::Snapshot(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            ));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = String::from("lendo snapshot…");
    }

    /// Guarda o token do formulário na memória local, chaveado pelo
    /// `node_url` do formulário (T-840-03c). Token vazio REMOVE a entrada.
    /// NUNCA vai ao catálogo nem a disco (RNF-04).
    pub fn save_token_from_form(&mut self) {
        let url = normalize_url(&self.form_node_url);
        if url.is_empty() {
            self.notice = String::from("node_url é obrigatório para guardar o token");
            return;
        }
        match optional_token(&self.form_token) {
            Some(token) => {
                self.tokens.insert(url.clone(), String::from(token));
                self.notice =
                    format!("token de {url} guardado só na memória (nunca vai ao catálogo)");
            }
            None => {
                if self.tokens.remove(&url).is_some() {
                    self.notice = format!("token de {url} removido da memória");
                } else {
                    self.notice = String::from("nenhum token para guardar");
                }
            }
        }
    }

    /// Publica a máquina do formulário no catálogo compartilhado (fluxo do
    /// `SharedCatalog`: publish condicional por base + releitura do
    /// snapshot). Conflito 409 (T-840-03c): o snapshot é relido, o aviso
    /// mostra a vigente e a base do formulário é preenchida — nada é
    /// aplicado silenciosamente. Se o formulário tem token, ele vai SÓ para
    /// a memória local (`tokens`), nunca para o valor publicado (RNF-04).
    ///
    /// # Errors
    /// Validação/serialização falham ANTES de any worker: `notice` registra
    /// o motivo e nada é publicado (retorna `Err` para o chamador decidir).
    pub fn publish_machine(&mut self) -> Result<(), MachinesError> {
        if self.busy {
            return Ok(());
        }
        let node_url = normalize_url(&self.form_node_url);
        let machine_id = self.form_machine_id.trim().to_string();
        if machine_id.is_empty() || node_url.is_empty() {
            self.notice = String::from("machine_id e node_url são obrigatórios");
            return Err(MachinesError::MissingIdentity);
        }
        if let Some(token) = optional_token(&self.form_token) {
            self.tokens.insert(node_url.clone(), String::from(token));
        }
        let host = {
            let explicit = self.form_host.trim();
            if explicit.is_empty() {
                // Design 3.10: o formulário traz só alias + URL — o host é
                // derivado da URL real (nada inventado).
                host_from_url(&node_url)
            } else {
                explicit.to_string()
            }
        };
        let machine = MachineRecord {
            machine_id,
            host,
            user: self.form_user.trim().to_string(),
            node_url,
            services_hint: non_empty(&self.form_services_hint),
            registered_at_ns: now_unix_ns(),
        };
        let value = machine_catalog_value(&machine).map_err(|err| {
            self.notice = format!("falha ao serializar máquina: {err}");
            MachinesError::Serialize(err.to_string())
        })?;
        let id = machine.catalog_id();
        let base = self.form_base.trim().parse::<u64>().ok();
        let url = self.url.clone();
        let auth = self.auth();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (notice, fill_base) =
                match publish_with_token(&url, &id, base, &value, auth.as_deref()) {
                    Ok(revision) => (format!("publicado em r{revision}"), None),
                    Err(err) => (
                        err.to_string(),
                        match &err {
                            SharedCatalogError::Conflict {
                                current: Some(current),
                            } => Some(*current),
                            _ => None,
                        },
                    ),
                };
            let then_snapshot = Some(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            );
            let _ = tx.send(MachinesMsg::Mutated {
                notice,
                fill_base,
                then_snapshot,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = format!("publicando máquina {}…", machine.machine_id);
        Ok(())
    }

    /// Remove a máquina do catálogo compartilhado (delete condicional
    /// pela revisão vigente no snapshot + releitura, mesmo fluxo do
    /// publish). Sem snapshot ou sem a máquina nele: aviso honesto e
    /// nada é enviado. O token em memória da URL é descartado junto.
    pub fn remove_machine(&mut self, machine_id: &str) {
        if self.busy {
            return;
        }
        let id = format!("{MACHINE_ID_PREFIX}{machine_id}");
        let base = self.snapshot.as_ref().and_then(|snapshot| {
            snapshot
                .items
                .iter()
                .find(|item| item.id.0 == id)
                .map(|item| item.revision.0)
        });
        let Some(base) = base else {
            self.notice =
                format!("máquina {machine_id} fora do snapshot atual — atualize antes de remover");
            return;
        };
        if let Some(machine) = self
            .machines()
            .into_iter()
            .find(|machine| machine.machine_id == machine_id)
        {
            self.tokens.remove(&normalize_url(&machine.node_url));
        }
        let url = self.url.clone();
        let auth = self.auth();
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let (notice, fill_base) = match delete_with_token(&url, &id, base, auth.as_deref()) {
                Ok(revision) => (format!("máquina removida em r{revision}"), None),
                Err(err) => (
                    err.to_string(),
                    match &err {
                        SharedCatalogError::Conflict {
                            current: Some(current),
                        } => Some(*current),
                        _ => None,
                    },
                ),
            };
            let then_snapshot = Some(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            );
            let _ = tx.send(MachinesMsg::Mutated {
                notice,
                fill_base,
                then_snapshot,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = format!("removendo máquina {machine_id}…");
    }
}
