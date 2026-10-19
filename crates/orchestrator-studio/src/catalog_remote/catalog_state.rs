//! Parte de `catalog_remote` (extraída para manter os arquivos revisáveis).

use super::*;

/// Estado do painel de catálogo compartilhado.
///
/// `token` (T-840-03a) vive só na memória da GUI — o `Debug` manual é
/// proposital: o valor NUNCA aparece em logs/derives (RNF-04).
#[derive(Default)]
pub struct SharedCatalog {
    pub url: String,
    pub snapshot: Option<Snapshot>,
    /// Cursor do log já incorporado à visão local (T-830-02): derivado do
    /// último snapshot aplicado ou do último lote só de exclusões.
    pub cursor: u64,
    pub form_id: String,
    pub form_value: String,
    pub form_base: String,
    pub notice: String,
    /// Token da autoridade (T-840-03a): vazio = sem token (modo localhost).
    pub token: String,
    /// `true` enquanto há HTTP em background (`poll` drena e libera).
    pub busy: bool,
    /// Filtro por kind (prefixo do id) da tabela — tela 3.9.
    pub kind_filter: Option<String>,
    /// Busca livre por chave ou payload — tela 3.9.
    pub search: String,
    /// Gaveta de publicação recolhida — tela 3.9.
    pub drawer_collapsed: bool,
    /// Conflito OCC vigente (banner 409) — limpo ao ressincronizar.
    pub conflict: Option<ConflictInfo>,
    /// Tombstones aplicados nesta sessão (nossos + acompanhados).
    pub session_tombstones: u64,
    /// Feed das últimas mutações (tela 3.9; mais novo no fim; máx 16).
    pub feed: Vec<FeedEntry>,
    /// `force_sync` encadeia follow após o snapshot chegar.
    pub(crate) follow_after: bool,
    pub(crate) receiver: Option<mpsc::Receiver<CatalogMsg>>,
}

impl std::fmt::Debug for SharedCatalog {
    fn fmt(&self, f: &mut std::fmt::Formatter<'_>) -> std::fmt::Result {
        f.debug_struct("SharedCatalog")
            .field("url", &self.url)
            .field("snapshot", &self.snapshot)
            .field("cursor", &self.cursor)
            .field("form_id", &self.form_id)
            .field("form_value", &self.form_value)
            .field("form_base", &self.form_base)
            .field("notice", &self.notice)
            .field("token_definido", &!self.token.trim().is_empty())
            .field("busy", &self.busy)
            .field("conflict", &self.conflict)
            .field("feed_len", &self.feed.len())
            .field("receiver", &self.receiver.is_some())
            .finish()
    }
}

impl SharedCatalog {
    /// Padrão honesto: autoridade local, sem token.
    #[must_use]
    pub fn with_url(url: &str) -> Self {
        Self {
            url: String::from(url),
            ..Self::default()
        }
    }

    /// Token efetivo da autoridade (campo de UI → `Option`).
    pub(super) fn auth(&self) -> Option<&str> {
        optional_token(&self.token)
    }

    /// Relê o snapshot em background; erro vira aviso, nunca linhas
    /// inventadas. Clique durante `busy` é ignorado.
    pub fn refresh(&mut self) {
        if self.busy {
            return;
        }
        let url = self.url.clone();
        let auth = self.auth().map(String::from);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let _ = tx.send(CatalogMsg::Snapshot(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            ));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = String::from("lendo snapshot…");
    }

    /// Publica o formulário (base vazia = criação) e relê em seguida — em
    /// background (REQ/T-820-19). Clique durante `busy` é ignorado.
    pub fn publish_form(&mut self) {
        if self.busy {
            return;
        }
        let base = self.form_base.trim().parse::<u64>().ok();
        let url = self.url.clone();
        let id = self.form_id.trim().to_string();
        let value = self.form_value.clone();
        let auth = self.auth().map(String::from);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = publish_with_token(&url, &id, base, &value, auth.as_deref());
            let notice = match &result {
                Ok(revision) => format!("publicado em r{revision}"),
                Err(err) => err.to_string(),
            };
            let then_snapshot = Some(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            );
            let _ = tx.send(CatalogMsg::Mutated {
                notice,
                then_snapshot,
                id,
                base,
                is_delete: false,
                result,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = String::from("publicando…");
    }

    /// Exclui o id do formulário com a base dada e relê em seguida — em
    /// background (REQ/T-820-19). Clique durante `busy` é ignorado.
    pub fn delete_form(&mut self) {
        if self.busy {
            return;
        }
        let base = self.form_base.trim().parse::<u64>().unwrap_or(u64::MAX);
        let url = self.url.clone();
        let id = self.form_id.trim().to_string();
        let auth = self.auth().map(String::from);
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let result = delete_with_token(&url, &id, base, auth.as_deref());
            let notice = match &result {
                Ok(revision) => format!("removido em r{revision}"),
                Err(err) => err.to_string(),
            };
            let then_snapshot = Some(
                fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
            );
            let _ = tx.send(CatalogMsg::Mutated {
                notice,
                then_snapshot,
                id,
                base: Some(base),
                is_delete: true,
                result,
            });
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = String::from("removendo…");
    }
}
