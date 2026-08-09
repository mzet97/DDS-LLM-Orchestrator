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
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::{Arc, Mutex};
use std::time::Duration;
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
    /// Nº de vezes que o driver se recuperou de um erro transitório em
    /// `wait_async` (ex.: `PRECONDITION_NOT_MET` quando o waitset fica
    /// momentaneamente sem entidades anexadas, sob corrida de attach/detach
    /// concorrente) — observabilidade/testes. Ver [`SharedWaitSet::driver_restarts`].
    driver_restarts: Arc<AtomicU64>,
}

impl SharedWaitSet {
    /// Cria o WaitSet compartilhado e o driver (1 task; 1 thread de
    /// blocking-pool ocupada por ciclo de `wait_async`, não por stream).
    pub fn new(participant: &impl DdsEntity) -> DdsResult<Arc<Self>> {
        let waitset = Arc::new(WaitSet::new(participant.entity())?);
        let notifiers: Arc<DashMap<i64, Arc<Notify>>> = Arc::new(DashMap::new());
        let driver_restarts = Arc::new(AtomicU64::new(0));

        let driver_waitset = Arc::clone(&waitset);
        let driver_notifiers = Arc::clone(&notifiers);
        let driver_restarts_task = Arc::clone(&driver_restarts);
        let driver = tokio::spawn(async move {
            // Um `Err` de `wait_async` NÃO implica shutdown intencional: se o
            // WaitSet foi realmente deletado (`Drop for SharedWaitSet`), o
            // `abort()` já teria cancelado esta future no próximo ponto de
            // suspensão, sem chance de cair neste `match`. Na prática, o `Err`
            // mais provável aqui é `PRECONDITION_NOT_MET` do CycloneDDS quando
            // o waitset fica momentaneamente com ZERO entidades anexadas —
            // uma corrida real sob carga: todos os `Registration`s ativos
            // podem ser dropados no mesmo instante em que um novo `register()`
            // ainda não anexou o seu. Antes, esse `Err` matava o driver
            // silenciosamente para sempre (só `debug!`), o que parava TODO o
            // fan-out de notificação do processo — o sintoma observado era
            // caches (`stream_tasks`/`stream_task_outputs`) que "às vezes
            // param de atualizar" e o endpoint `/sync` "travando"
            // intermitentemente sob carga concorrente. Agora: nunca desiste
            // sozinho, só espera um pouco (backoff) e tenta de novo — a única
            // forma de parar esta task é `abort()` externo, em `Drop`.
            let base_backoff = Duration::from_millis(5);
            let max_backoff = Duration::from_millis(500);
            let mut backoff = base_backoff;
            loop {
                match driver_waitset.wait_async(i64::MAX).await {
                    Ok(cookies) => {
                        backoff = base_backoff; // reseta após qualquer sucesso
                        for cookie in cookies {
                            if let Some(n) = driver_notifiers.get(&cookie) {
                                n.notify_one();
                            }
                        }
                    }
                    Err(e) => {
                        driver_restarts_task.fetch_add(1, Ordering::Relaxed);
                        tracing::warn!(
                            error = %e,
                            backoff_ms = backoff.as_millis(),
                            "SharedWaitSet: wait_async falhou (provável corrida de \
                             attach/detach com o waitset momentaneamente vazio); \
                             retomando após backoff, driver NÃO encerra"
                        );
                        tokio::time::sleep(backoff).await;
                        backoff = (backoff * 2).min(max_backoff);
                    }
                }
            }
        });

        Ok(Arc::new(Self {
            waitset,
            next_cookie: AtomicI64::new(1),
            notifiers,
            driver: Mutex::new(Some(driver)),
            driver_restarts,
        }))
    }

    /// Nº de vezes que o driver se recuperou de um erro transitório em
    /// `wait_async` desde a criação deste `SharedWaitSet`. Deveria ser 0 em
    /// operação normal; um valor crescente durante uma campanha experimental
    /// indica a corrida de attach/detach descrita em `new()` — útil para
    /// confirmar (ou descartar) essa causa em produção sem precisar
    /// reproduzir localmente.
    pub fn driver_restarts(&self) -> u64 {
        self.driver_restarts.load(Ordering::Relaxed)
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
    /// Future de notificação deste reader. Para loops de consumo, o padrão
    /// correto (sem janela de wakeup perdido) é registrar interesse ANTES de
    /// drenar e drenar até esvaziar:
    ///
    /// ```ignore
    /// loop {
    ///     let n = registration.notified();
    ///     tokio::pin!(n);
    ///     n.as_mut().enable();           // registra antes de drenar
    ///     while take()?.not_empty { yield } // drena tudo (level-triggered)
    ///     n.await;                       // notificações durante o dreno são capturadas
    /// }
    /// ```
    pub fn notified(&self) -> tokio::sync::futures::Notified<'_> {
        self.notify.notified()
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
