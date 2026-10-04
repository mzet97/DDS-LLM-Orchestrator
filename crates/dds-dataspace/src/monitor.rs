//! Monitor de QoS nativo (T-306, REQ-307/308).
//!
//! Listeners do CycloneDDS (`on_liveliness_changed`, `on_requested_deadline_missed`)
//! em vez do polling do `QoSMonitor` Python — eventos chegam via `broadcast`.

use cyclonedds::{DdsResult, Listener};
use std::sync::atomic::{AtomicI64, AtomicU64, Ordering};
use std::sync::Arc;
use tokio::sync::broadcast;

/// Evento de QoS observado pelo monitor.
#[derive(Debug, Clone)]
pub enum QosEvent {
    /// Mudança de liveliness de writers (chegada/saída de agentes).
    LivelinessChanged {
        alive: u32,
        not_alive: u32,
        alive_delta: i32,
        not_alive_delta: i32,
    },
    /// Deadline de escrita perdido pelo lado leitor (ex.: stream travou).
    DeadlineMissed { total: i32, delta: i32 },
}

/// Monitor: instala listeners e repassa eventos por canal broadcast.
pub struct QosMonitor {
    tx: broadcast::Sender<QosEvent>,
    missed_total: Arc<AtomicU64>,
    /// Saldo líquido de writers VIVOS — acumula SOMENTE `alive_count_change`
    /// (T-820-05/P1-4: somar também `not_alive_count_change` fazia o saldo
    /// zerar exatamente na transição que importa — a morte de um agente, que
    /// emite +1 alive/-1 not_alive no mesmo callback — tornando a métrica
    /// morta para o evento que ela deveria medir).
    alive_net: Arc<AtomicI64>,
    /// Total acumulado de writers detectados como NÃO vivos (lease de
    /// liveliness expirado) — o sinal de "agente morreu" que o saldo acima
    /// não esconde.
    not_alive_net: Arc<AtomicI64>,
}

impl Default for QosMonitor {
    fn default() -> Self {
        Self::new()
    }
}

impl QosMonitor {
    pub fn new() -> Self {
        let (tx, _) = broadcast::channel(256);
        Self {
            tx,
            missed_total: Arc::new(AtomicU64::new(0)),
            alive_net: Arc::new(AtomicI64::new(0)),
            not_alive_net: Arc::new(AtomicI64::new(0)),
        }
    }

    /// Assinatura dos eventos (cada chamada cria um receiver).
    pub fn subscribe(&self) -> broadcast::Receiver<QosEvent> {
        self.tx.subscribe()
    }

    /// Total de deadlines perdidos observados.
    pub fn deadlines_missed(&self) -> u64 {
        self.missed_total.load(Ordering::Relaxed)
    }

    /// Saldo de writers VIVOS (chegadas - saídas, só `alive_count_change`).
    /// Análogo ao `last_seen` do reaper: cai quando um agente morre.
    pub fn alive_writers_net(&self) -> i64 {
        self.alive_net.load(Ordering::Relaxed)
    }

    /// Total de writers detectados como não vivos (lease expirado) desde a
    /// criação do monitor — incrementa a cada morte de agente observada
    /// (T-820-05/P1-4).
    pub fn not_alive_writers_net(&self) -> i64 {
        self.not_alive_net.load(Ordering::Relaxed)
    }

    /// Listener para o reader de `AgentRegistry` (liveliness dos agentes).
    /// Retorna `Err` (ex.: `OutOfResources`) em vez de panic — M3.
    pub fn agents_listener(&self) -> DdsResult<Listener> {
        let tx = self.tx.clone();
        let alive_net = Arc::clone(&self.alive_net);
        let not_alive_net = Arc::clone(&self.not_alive_net);
        Listener::builder()
            .on_liveliness_changed(move |_e, s| {
                alive_net.fetch_add(s.alive_count_change as i64, Ordering::Relaxed);
                not_alive_net.fetch_add(s.not_alive_count_change as i64, Ordering::Relaxed);
                let _ = tx.send(QosEvent::LivelinessChanged {
                    alive: s.alive_count,
                    not_alive: s.not_alive_count,
                    alive_delta: s.alive_count_change,
                    not_alive_delta: s.not_alive_count_change,
                });
            })
            .build()
    }

    /// Listener para o reader de `TaskOutput` (deadline missed).
    /// Retorna `Err` (ex.: `OutOfResources`) em vez de panic — M3.
    pub fn outputs_listener(&self) -> DdsResult<Listener> {
        let tx = self.tx.clone();
        let missed = Arc::clone(&self.missed_total);
        Listener::builder()
            .on_requested_deadline_missed(move |_e, s| {
                missed.fetch_add(s.total_count_change as u64, Ordering::Relaxed);
                let _ = tx.send(QosEvent::DeadlineMissed {
                    total: s.total_count as i32,
                    delta: s.total_count_change,
                });
            })
            .build()
    }
}
