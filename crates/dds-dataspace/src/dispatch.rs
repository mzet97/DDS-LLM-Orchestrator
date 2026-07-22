//! WaitSet compartilhado (Fase 5 / T-617 do `ACTION_PLAN_DDS_IMPLEMENTATION.md`).
//!
//! Antes: cada `stream_*()` chamava `reader.take_aiter()`, que cria seu PRÓPRIO
//! `WaitSet` e ocupa uma thread do blocking-pool do tokio (via `spawn_blocking`)
//! por toda a vida da stream, bloqueada em `dds_waitset_wait`. Com N streams
//! ativas (ex.: o `client` abre 2 por `submit()` — `stream_tasks` +
//! `stream_task_outputs` — e cada `submit()` concorrente é uma stream nova; o
//! cenário de 50 clientes concorrentes já validado em `specs/300-control-plane`
//! significa até 100 WaitSets simultâneos), isso são até N threads de
//! blocking-pool permanentemente ocupadas só esperando.
//!
//! Agora: UM `WaitSet` por `DataSpace`, compartilhado por todas as streams.
//! Cada `stream_*()` continua com seu PRÓPRIO `DataReader` (preserva a
//! semântica atual de N assinantes independentes por tópico — cada um vê
//! TODAS as amostras via seu próprio `dds_take`, sem fan-out/broadcast e sem
//! corrida de leitura entre assinantes), mas em vez de criar seu próprio
//! WaitSet, ANEXA esse reader ao WaitSet compartilhado (cookie único por
//! anexação) e espera por uma notificação local (`tokio::sync::Notify`) em vez
//! de bloquear uma thread própria. Um único driver (1 task, 1 thread de
//! blocking-pool por ciclo de `wait_async`) drena os cookies disparados e
//! notifica só os registros correspondentes.

use cyclonedds::{DdsEntity, DdsResult, WaitSet};
use dashmap::DashMap;
use std::sync::atomic::{AtomicI64, Ordering};
use std::sync::{Arc, Mutex};
use tokio::sync::Notify;

/// WaitSet único por `DataSpace`, compartilhado por todos os `stream_*()`.
///
/// A condição de dados disponíveis em DDS é *level-triggered* (permanece
/// disparada enquanto houver amostra não lida) — por isso um `notify_one()`
/// "perdido" (o `Notify` só retém 1 permit) nunca trava um consumidor: o
/// driver volta a disparar aquele cookie no próximo ciclo de `wait_async`
/// enquanto a condição continuar verdadeira.
pub struct SharedWaitSet {
    waitset: Arc<WaitSet>,
    next_cookie: AtomicI64,
    notifiers: Arc<DashMap<i64, Arc<Notify>>>,
    driver: Mutex<Option<tokio::task::JoinHandle<()>>>,
}

impl SharedWaitSet {
    /// Cria o WaitSet compartilhado e o driver (1 task; 1 thread de
    /// blocking-pool ocupada por ciclo de `wait_async`, não por stream).
    pub fn new(participant: &impl DdsEntity) -> DdsResult<Arc<Self>> {
        let waitset = Arc::new(WaitSet::new(participant.entity())?);
        let notifiers: Arc<DashMap<i64, Arc<Notify>>> = Arc::new(DashMap::new());

        let driver_waitset = Arc::clone(&waitset);
        let driver_notifiers = Arc::clone(&notifiers);
        let driver = tokio::spawn(async move {
            loop {
                match driver_waitset.wait_async(i64::MAX).await {
                    Ok(cookies) => {
                        for cookie in cookies {
                            if let Some(n) = driver_notifiers.get(&cookie) {
                                n.notify_one();
                            }
                        }
                    }
                    Err(e) => {
                        // WaitSet provavelmente deletado (DataSpace::shutdown/drop) —
                        // encerra o driver em vez de girar em erro.
                        tracing::debug!(error = %e, "SharedWaitSet: driver encerrando");
                        break;
                    }
                }
            }
        });

        Ok(Arc::new(Self {
            waitset,
            next_cookie: AtomicI64::new(1),
            notifiers,
            driver: Mutex::new(Some(driver)),
        }))
    }

    /// Anexa `reader` ao WaitSet compartilhado. O [`Registration`] devolvido
    /// permite esperar (`notified().await`) por dados nesse reader
    /// especificamente; ao ser dropado, desanexa do WaitSet e libera o cookie
    /// (RAII — nenhuma limpeza manual necessária no chamador).
    /// Nº de registros (readers anexados) ativos agora — só para
    /// observabilidade/testes: prova que N streams compartilham 1 WaitSet em
    /// vez de N WaitSets independentes (ver `tests/shared_waitset.rs`).
    pub fn registration_count(&self) -> usize {
        self.notifiers.len()
    }

    pub fn register(&self, reader: &impl DdsEntity) -> DdsResult<Registration> {
        let entity = reader.entity();
        let cookie = self.next_cookie.fetch_add(1, Ordering::Relaxed);
        let notify = Arc::new(Notify::new());
        self.notifiers.insert(cookie, Arc::clone(&notify));
        if let Err(e) = self.waitset.attach(entity, cookie) {
            self.notifiers.remove(&cookie);
            return Err(e);
        }
        Ok(Registration {
            waitset: Arc::clone(&self.waitset),
            notifiers: Arc::clone(&self.notifiers),
            cookie,
            entity,
            notify,
        })
    }
}

impl Drop for SharedWaitSet {
    fn drop(&mut self) {
        // `abort()`, não só dropar o handle: um `JoinHandle` dropado sem abort
        // apenas desanexa (`detach`) a task — ela continuaria rodando para
        // sempre em background, seu clone de `Arc<WaitSet>` mantendo o
        // WaitSet nativo vivo mesmo depois do `DataSpace` cair. `abort()`
        // cancela a task no próximo ponto de suspensão (`.await` dentro do
        // loop), que solta seu `Arc<WaitSet>` ao ser dropada.
        if let Some(handle) = self.driver.lock().unwrap().take() {
            handle.abort();
        }
    }
}

/// Anexação de um reader ao [`SharedWaitSet`]. Enquanto viva, `notified()`
/// resolve toda vez que o WaitSet compartilhado detecta dados nesse reader
/// especificamente. Ao ser dropada, desanexa do WaitSet e libera o cookie.
pub struct Registration {
    waitset: Arc<WaitSet>,
    notifiers: Arc<DashMap<i64, Arc<Notify>>>,
    cookie: i64,
    entity: i32,
    notify: Arc<Notify>,
}

impl Registration {
    pub async fn notified(&self) {
        self.notify.notified().await;
    }
}

impl Drop for Registration {
    fn drop(&mut self) {
        // Ignora erro: se o WaitSet/reader já foi deletado (shutdown em
        // andamento), não há o que desanexar.
        let _ = self.waitset.detach(self.entity);
        self.notifiers.remove(&self.cookie);
    }
}
