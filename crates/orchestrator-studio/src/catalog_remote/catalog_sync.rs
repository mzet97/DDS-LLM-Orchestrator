//! Parte de `catalog_remote` (extraída para manter os arquivos revisáveis).

use super::*;

impl SharedCatalog {
    /// Acompanha eventos desde o cursor local (T-830-02) — em background
    /// (REQ/T-820-19). Clique durante `busy` é ignorado. Lote só de
    /// `Deleted` aplica a remoção incrementalmente; qualquer `Created`/
    /// `Updated` (evento não carrega valor) ou 410 `CursorExpired` termina
    /// em snapshot fresco. Nó inalcançável: estado permanece, motivo vira
    /// aviso com o mesmo texto do erro tipado.
    pub fn follow_events(&mut self) {
        if self.busy {
            return;
        }
        let url = self.url.clone();
        let auth = self.auth().map(String::from);
        let cursor = self.cursor;
        let (tx, rx) = mpsc::channel();
        std::thread::spawn(move || {
            let outcome = match events_since_with_token(&url, cursor, auth.as_deref()) {
                Ok(events) if events.is_empty() => FollowOutcome::UpToDate,
                Ok(events) if events.iter().all(|event| event.kind == EventKind::Deleted) => {
                    FollowOutcome::Deleted(events)
                }
                Ok(events) => FollowOutcome::Applied {
                    events,
                    snapshot: fetch_snapshot_with_token(&url, auth.as_deref())
                        .map_err(|err| err.to_string()),
                },
                Err(SharedCatalogError::CursorExpired) => FollowOutcome::Resync(
                    fetch_snapshot_with_token(&url, auth.as_deref()).map_err(|err| err.to_string()),
                ),
                Err(err) => FollowOutcome::Failed(err.to_string()),
            };
            let _ = tx.send(CatalogMsg::Followed(outcome));
        });
        self.receiver = Some(rx);
        self.busy = true;
        self.notice = format!("acompanhando eventos desde o cursor {cursor}…");
    }

    /// Registra no feed com carimbo de relógio (máx 16; mais novo no fim).
    pub(super) fn feed_push(&mut self, tag: FeedTag, text: String) {
        self.feed.push(FeedEntry {
            ts_ms: crate::machines::now_unix_ns() / 1_000_000,
            tag,
            text,
        });
        if self.feed.len() > 16 {
            let drop = self.feed.len() - 16;
            self.feed.drain(0..drop);
        }
    }

    /// Snapshot + acompanhamento em sequência (botão Forçar Sincronização da
    /// 3.9): o follow encadeia quando o snapshot chegar (ver `poll`).
    pub fn force_sync(&mut self) {
        if self.busy {
            return;
        }
        self.follow_after = true;
        self.refresh();
    }

    /// Snapshot atual em JSON no temporário (botão Exportar JSON da 3.9).
    pub fn export_snapshot(&self) -> std::io::Result<std::path::PathBuf> {
        let snapshot = self
            .snapshot
            .as_ref()
            .ok_or_else(|| std::io::Error::new(std::io::ErrorKind::NotFound, "sem snapshot"))?;
        let payload = serde_json::to_string_pretty(snapshot).map_err(std::io::Error::other)?;
        let path = std::env::temp_dir().join("studio-catalog-snapshot.json");
        std::fs::write(&path, payload)?;
        Ok(path)
    }

    /// Novo registro: limpa o formulário e abre a gaveta (sem rede).
    pub fn new_entry(&mut self) {
        self.form_id.clear();
        self.form_value.clear();
        self.form_base.clear();
        self.drawer_collapsed = false;
        self.conflict = None;
        self.notice = String::from("formulário limpo para novo registro");
    }
}
