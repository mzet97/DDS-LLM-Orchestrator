//! Parte de `catalog_remote` (extraída para manter os arquivos revisáveis).

use super::*;

impl SharedCatalog {
    /// Drena o worker; chamar a cada frame enquanto `busy`.
    /// Nota: dentro de `poll` o snapshot é aplicado por atribuição direta de
    /// campos (`cursor` + `snapshot`) — um método `&mut self` conflitaria com
    /// o empréstimo de `receiver` no mesmo escopo.
    pub fn poll(&mut self) {
        let mut finished = false;
        let mut chain_follow = false;
        // Feed acumula local: `feed_push` (&mut self) conflitaria com o
        // empréstimo de `receiver` (E0502) — descarrega após o bloco.
        let mut pending_feed: Vec<(FeedTag, String)> = Vec::new();
        if let Some(rx) = &self.receiver {
            while let Ok(msg) = rx.try_recv() {
                match msg {
                    CatalogMsg::Snapshot(result) => match result {
                        Ok(snapshot) => {
                            self.cursor = snapshot.cursor.0;
                            let items = snapshot.items.len();
                            self.snapshot = Some(snapshot);
                            self.conflict = None;
                            pending_feed.push((
                                FeedTag::Snapshot,
                                format!("{items} iten(s) · cursor {}", self.cursor),
                            ));
                            self.notice.clear();
                            chain_follow = self.follow_after;
                            self.follow_after = false;
                        }
                        Err(err) => {
                            self.follow_after = false;
                            self.notice = err;
                        }
                    },
                    CatalogMsg::Mutated {
                        notice,
                        then_snapshot,
                        id,
                        base,
                        is_delete,
                        result,
                    } => {
                        match &result {
                            Ok(revision) => {
                                self.conflict = None;
                                if is_delete {
                                    self.session_tombstones += 1;
                                    pending_feed.push((
                                        FeedTag::Tombstone,
                                        format!("tombstone rev.{revision} para {id}"),
                                    ));
                                } else {
                                    pending_feed.push((
                                        FeedTag::Committed,
                                        format!("rev.{revision} gravada para {id}"),
                                    ));
                                }
                            }
                            Err(SharedCatalogError::Conflict { current }) => {
                                self.conflict = Some(ConflictInfo {
                                    attempted_base: base,
                                    current: *current,
                                });
                                pending_feed.push((
                                    FeedTag::Rejected,
                                    format!(
                                        "{id}: base {} expirada; vigente {}",
                                        base.map_or(String::from("—"), |b| format!("r{b}")),
                                        current.map_or(String::from("—"), |c| format!("r{c}")),
                                    ),
                                ));
                            }
                            Err(_) => {
                                self.conflict = None;
                            }
                        }
                        self.notice = notice;
                        if let Some(result) = then_snapshot {
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
                        }
                    }
                    CatalogMsg::Followed(outcome) => match outcome {
                        FollowOutcome::UpToDate => {
                            self.notice = format!("sem eventos novos (cursor {})", self.cursor);
                        }
                        FollowOutcome::Deleted(events) => {
                            let removed: Vec<DefinitionId> =
                                events.iter().map(|event| event.id.clone()).collect();
                            for event in &events {
                                self.session_tombstones += 1;
                                pending_feed.push((
                                    FeedTag::Tombstone,
                                    format!(
                                        "seq{} · {} rev.{}",
                                        event.seq, event.id.0, event.revision.0
                                    ),
                                ));
                            }
                            if let Some(snapshot) = &mut self.snapshot {
                                // Id excluído ganha tombstone no nó: nunca
                                // volta, então toda linha com o id sai.
                                snapshot.items.retain(|item| !removed.contains(&item.id));
                            }
                            if let Some(last) = events.last() {
                                self.cursor = last.seq + 1;
                            }
                            self.notice = format!(
                                "{} exclusão(ões) aplicada(s) (cursor {})",
                                events.len(),
                                self.cursor
                            );
                        }
                        FollowOutcome::Applied { events, snapshot } => {
                            let count = events.len();
                            for event in &events {
                                let tag = if event.kind == EventKind::Deleted {
                                    self.session_tombstones += 1;
                                    FeedTag::Tombstone
                                } else {
                                    FeedTag::Committed
                                };
                                pending_feed.push((
                                    tag,
                                    format!(
                                        "seq{} · {} rev.{}",
                                        event.seq, event.id.0, event.revision.0
                                    ),
                                ));
                            }
                            match snapshot {
                                Ok(fresh) => {
                                    let cursor = fresh.cursor.0;
                                    self.snapshot = Some(fresh);
                                    self.cursor = cursor;
                                    self.notice = format!(
                                        "{count} novo(s) evento(s) aplicado(s) (cursor {cursor})"
                                    );
                                }
                                Err(err) => {
                                    self.notice = err;
                                }
                            }
                        }
                        FollowOutcome::Resync(snapshot) => match snapshot {
                            Ok(fresh) => {
                                let cursor = fresh.cursor.0;
                                self.snapshot = Some(fresh);
                                self.cursor = cursor;
                                self.notice = format!(
                                    "cursor expirado; snapshot reaplicado (cursor {cursor})"
                                );
                            }
                            Err(err) => {
                                self.notice = err;
                            }
                        },
                        FollowOutcome::Failed(err) => {
                            self.notice = err;
                        }
                    },
                }
                finished = true;
            }
        }
        if finished {
            self.receiver = None;
            self.busy = false;
        }
        for (tag, text) in pending_feed {
            self.feed_push(tag, text);
        }
        // `force_sync`: o follow só dispara com o worker livre.
        if chain_follow && !self.busy {
            self.follow_events();
        }
    }
}
