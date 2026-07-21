//! Serviço DDS do Context Store (feature `dds`) — port de
//! `context_store/main.py::ContextStoreService`.
//!
//! Assina `Context.Snapshot` e `Context.Update` pelas streams por evento do
//! `DataSpace` (WaitSet via `take_aiter`, sem polling) e alimenta o store:
//! snapshots viram `put_snapshot` (upsert), updates viram `apply_update`
//! (merge de delta). O Python assinava apenas `Context.Update`; assinar
//! `Context.Snapshot` também fecha o ciclo completo do contrato.
//!
//! Erros por amostra (ex.: delta com JSON inválido) são logados e a amostra é
//! descartada — paridade com o Python, onde a exceção da corotina
//! `apply_update` ficava contida no `run_coroutine_threadsafe` e não derrubava
//! o serviço.

use crate::store::ContextStore;
use dds_dataspace::api::DataSpaceError;
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::sync::Arc;

/// Serviço Context Store: consome os tópicos Context.* e persiste no store.
pub struct ContextStoreService<S: ContextStore> {
    dataspace: Arc<DataSpace>,
    store: Arc<S>,
    domain_id: u32,
}

impl<S: ContextStore> ContextStoreService<S> {
    /// Sobe o DataSpace no domínio (o serviço só lê os tópicos Context; o
    /// strength segue o papel orquestrador, como os demais serviços core).
    pub fn new(domain_id: u32, store: Arc<S>) -> Result<Self, DataSpaceError> {
        let dataspace = Arc::new(DataSpace::new(domain_id, DataSpace::STRENGTH_ORCHESTRATOR)?);
        Ok(Self {
            dataspace,
            store,
            domain_id,
        })
    }

    /// Handle do DataSpace (diagnóstico/testes).
    pub fn dataspace(&self) -> &Arc<DataSpace> {
        &self.dataspace
    }

    /// Handle do store (leituras/de diagnóstico).
    pub fn store(&self) -> &Arc<S> {
        &self.store
    }

    /// Roda até `shutdown` completar (Ctrl+C/SIGTERM no binário) ou ambas as
    /// streams fecharem. Cada amostra recebida alimenta o store.
    pub async fn run(&self, shutdown: impl std::future::Future<Output = ()> + Send) {
        let mut snapshots = Box::pin(self.dataspace.stream_context_snapshots());
        let mut updates = Box::pin(self.dataspace.stream_context_updates());
        let mut shutdown = Box::pin(shutdown);
        tracing::info!(domain_id = self.domain_id, "Context Store iniciado");

        loop {
            tokio::select! {
                _ = &mut shutdown => {
                    tracing::info!("shutdown solicitado — encerrando");
                    break;
                }
                item = snapshots.next() => {
                    match item {
                        Some(snap) => {
                            if let Err(e) = self.store.put_snapshot(&snap).await {
                                tracing::error!(
                                    context_id = %snap.context_id,
                                    error = %e,
                                    "falha ao persistir snapshot — amostra descartada"
                                );
                            }
                        }
                        None => {
                            tracing::warn!("stream Context.Snapshot fechada");
                            break;
                        }
                    }
                }
                item = updates.next() => {
                    match item {
                        Some(update) => {
                            if let Err(e) = self.store.apply_update(&update).await {
                                tracing::error!(
                                    context_id = %update.context_id,
                                    update_type = update.update_type,
                                    error = %e,
                                    "falha ao aplicar update — amostra descartada"
                                );
                            }
                        }
                        None => {
                            tracing::warn!("stream Context.Update fechada");
                            break;
                        }
                    }
                }
            }
        }
        tracing::info!("Context Store encerrado");
    }
}
