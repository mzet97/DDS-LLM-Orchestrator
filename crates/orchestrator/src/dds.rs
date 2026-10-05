//! Runtime DDS do orchestrator (T-401/T-403/T-405).
//!
//! - `publish_task`: API → Task no tópico `Tasks` (agentes claim — data-centric).
//! - `spawn_registry_monitor`: assina AgentRegistry + liveliness; agente morto
//!   reatribui suas tasks não-terminais para PENDING e publica
//!   `QoS.Violation("liveliness_lost")`. T-820-03: a reatribuição é publicada
//!   SEM ownership (strength de cliente) e passa por `state_machine::reassign`
//!   (teto de retries + renovação de `created_at_ns`); o reaper de progresso
//!   recolhe tasks ASSIGNED/RUNNING estagnadas de agentes vivos.
//! - `spawn_control_loop`: NFCM decide perfil QoS periodicamente e aplica os
//!   knobs online (TransportPriority/LatencyBudget/OwnershipStrength) no writer
//!   de Tasks; cada decisão é tracejada (`qos_decision`).
//! - `spawn_qos_monitor`: porte de `QoSMonitor.run()` (`dds_backend/qos_monitor.py`)
//!   — deadlines de tasks expiradas viram `QoS.Violation`; os contadores viram
//!   `QoS.Metric` periodicamente. Ver módulo [`crate::qos_monitor`] para o porte
//!   completo e a nota sobre o que NÃO foi portado (os 8 listeners nativos de
//!   violação do Python nunca chegaram a ser conectados a readers/writers reais
//!   em produção — só a detecção por polling roda de fato).

use crate::{AgentRegistry, Scheduler};
use anyhow::Result;
use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use futures_util::StreamExt;
use orch_common::FuzzyMetrics;
use qos_nfcm::decider::{QoSDecision, QosDecider};
use std::sync::Arc;
use std::time::Duration;
use tokio::sync::RwLock;

fn now_ns() -> u64 {
    std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// T-820-03: teto de reatribuições por task. Ao atingi-lo, `state_machine::
/// reassign` transiciona para FAILED("MAX_RETRIES_EXCEEDED") em vez de
/// devolver a task para PENDING — sem isso uma task repetidamente estagnada
/// ficaria em retry infinito (o cliente nunca observaria terminalidade).
const MAX_TASK_RETRIES: u32 = 3;

/// T-820-03: janela de estagnação de tasks ASSIGNED para o reaper de
/// progresso (agentes VIVOS). Fecha o vazamento de confirmação
/// falso-negativa (`agent/src/dds.rs`: readback do claim perdido → agente
/// desiste, mas o ASSIGNED dele pode vencer a arbitragem depois → task presa
/// em ASSIGNED para sempre, pois `is_eligible` filtra status ≠ PENDING e o
/// reaper de liveliness só enxerga agentes mortos). 30 s ≫ `CONFIRM_DELAY`
/// (250 ms) + `CONFIRM_TIMEOUT` (10 s) do agente: qualquer tentativa de
/// claim legítima resolve dentro da janela.
const ASSIGNED_STALL_AFTER: Duration = Duration::from_secs(30);

/// T-820-03: janela de estagnação de tasks RUNNING para o reaper de
/// progresso. 60 s cobre inferências longas legítimas (o agente deriva seu
/// timeout do `deadline_ns` da task, tipicamente 120 s) sem reciclar tasks
/// saudáveis — o custo de um falso positivo é apenas retry_count+1.
const RUNNING_STALL_AFTER: Duration = Duration::from_secs(60);

/// Runtime do orchestrator sobre o DataSpace real.
pub struct OrchestratorDds {
    dataspace: Arc<DataSpace>,
    /// Writer da API para submissões de clientes (strength 10 — papel cliente;
    /// se fosse 200, os claims dos agentes (100) perderiam a arbitragem).
    api_tasks_writer: cyclonedds::DataWriter<Task>,
    registry: Arc<AgentRegistry>,
    scheduler: Arc<RwLock<Scheduler>>,
    decider: Arc<dyn QosDecider>,
    metrics: Arc<parking_lot::RwLock<FuzzyMetrics>>,
    decisions: Arc<std::sync::atomic::AtomicU64>,
    last_seen: Arc<dashmap::DashMap<String, std::time::Instant>>,
    /// `--fuzzy-routing` (default OFF — paridade com `enable_fuzzy_routing`).
    fuzzy_routing: bool,
    /// `_routing_profile_version`/`_last_routing_profile_name` do Python:
    /// versão só incrementa quando o perfil publicado muda.
    routing_version: std::sync::atomic::AtomicI32,
    last_routing_profile: parking_lot::Mutex<String>,
    /// `QoSMonitor.counters`/`window_deltas` (dict+lock no Python) — total
    /// acumulado e delta desde a última publicação, por `metric_name`.
    qos_counters: dashmap::DashMap<String, i64>,
    qos_window_deltas: dashmap::DashMap<String, i32>,
    /// `_reported_deadlines` do Python: dedup de deadline já reportado por
    /// `task_id`, com o mesmo limite/estratégia de eviction crua (clear ao
    /// exceder `REPORTED_DEADLINES_MAX`).
    reported_deadlines: dashmap::DashSet<String>,
    qos_last_publish_ns: std::sync::atomic::AtomicU64,
    /// T-820-03: reatribuições aguardando visibilidade no mesh. O write de
    /// reatribuição sai em strength de CLIENTE (10) e, sob Exclusive
    /// Ownership, é descartado pelo RHC enquanto o dono anterior (ASSIGNED/
    /// RUNNING do agente, strength 100+) seguir registrado — o ownership só
    /// é liberado quando o writer do dono é DESTRUÍDO
    /// (`dds_rhc_default_unregister_wr` → `relinquish_ownership`; num kill
    /// real isso leva o lease do DDSI). O republisher re-publica a cada tick
    /// até o mesh refletir o PENDING (ver `republishe_reatribuicoes_pendentes`).
    pending_reassignment: dashmap::DashMap<String, (Task, std::time::Instant)>,
    /// EXP1b (dissertação §3.7.4): `--dispatch-mode` — despacho central
    /// emulado via `target_agent` (quem decide a atribuição passa a ser o
    /// orquestrador; claim+readback do agente permanecem).
    dispatch_mode: std::sync::atomic::AtomicBool,
}

impl OrchestratorDds {
    /// Sobe o runtime (orquestrador = papel strength 200 para reaper/failover).
    /// O decisor de QoS vem do `--qos-manager` (T-504).
    /// `qos_profile`: perfil QoS estrutural para a campanha experimental (None = default).
    pub fn new(
        domain_id: u32,
        decider: Arc<dyn QosDecider>,
        qos_profile: Option<&str>,
    ) -> Result<Self> {
        let dataspace = Arc::new(DataSpace::new_with_profile(
            domain_id,
            DataSpace::STRENGTH_ORCHESTRATOR,
            qos_profile,
        )?);
        Self::build(dataspace, decider)
    }

    #[cfg(feature = "security")]
    pub fn new_with_security(
        domain_id: u32,
        decider: Arc<dyn QosDecider>,
        qos_profile: Option<&str>,
        security: Option<dds_dataspace::SecurityConfig>,
    ) -> Result<Self> {
        let dataspace = Arc::new(DataSpace::new_with_profile_and_security(
            domain_id,
            DataSpace::STRENGTH_ORCHESTRATOR,
            qos_profile,
            security,
        )?);
        Self::build(dataspace, decider)
    }

    fn build(dataspace: Arc<DataSpace>, decider: Arc<dyn QosDecider>) -> Result<Self> {
        let api_qos = dds_dataspace::qos::profiles::tasks(Some(DataSpace::STRENGTH_CLIENT))?;
        let api_tasks_writer = dataspace.tasks_writer_with(&api_qos)?;
        Ok(Self {
            dataspace,
            api_tasks_writer,
            registry: Arc::new(AgentRegistry::new()),
            scheduler: Arc::new(RwLock::new(Scheduler::new())),
            decider,
            metrics: Arc::new(parking_lot::RwLock::new(FuzzyMetrics::default())),
            decisions: Arc::new(std::sync::atomic::AtomicU64::new(0)),
            last_seen: Arc::new(dashmap::DashMap::new()),
            fuzzy_routing: false,
            routing_version: std::sync::atomic::AtomicI32::new(0),
            last_routing_profile: parking_lot::Mutex::new(String::new()),
            qos_counters: dashmap::DashMap::new(),
            qos_window_deltas: dashmap::DashMap::new(),
            reported_deadlines: dashmap::DashSet::new(),
            qos_last_publish_ns: std::sync::atomic::AtomicU64::new(now_ns()),
            pending_reassignment: dashmap::DashMap::new(),
            dispatch_mode: std::sync::atomic::AtomicBool::new(false),
        })
    }

    /// Liga a publicação de `QoS.RoutingProfile` (porte de `--fuzzy-routing`).
    pub fn with_fuzzy_routing(mut self, enabled: bool) -> Self {
        self.fuzzy_routing = enabled;
        self
    }

    /// EXP1b: ativa o despacho central emulado (`--dispatch-mode`).
    pub fn with_dispatch_mode(self, enabled: bool) -> Self {
        self.dispatch_mode
            .store(enabled, std::sync::atomic::Ordering::Relaxed);
        self
    }

    /// EXP1b: despacho central emulado — escolhe o agente menos ocupado
    /// entre os disponíveis e fixa `target_agent` (task segue PENDING; o
    /// claim do agente permanece). Retorna `false` quando o modo está
    /// desligado ou não há agente disponível.
    pub fn dispatch_task(&self, task: &mut Task) -> bool {
        if !self
            .dispatch_mode
            .load(std::sync::atomic::Ordering::Relaxed)
        {
            return false;
        }
        let mut candidates = self.registry.available();
        if candidates.is_empty() {
            tracing::warn!(task_id = %task.task_id, "dispatch: sem agente disponível — task segue em claim aberto");
            return false;
        }
        candidates.sort_by_key(|a| a.slots_busy);
        task.target_agent = candidates.remove(0).agent_id;
        true
    }

    pub fn dataspace(&self) -> &Arc<DataSpace> {
        &self.dataspace
    }

    pub fn registry(&self) -> &Arc<AgentRegistry> {
        &self.registry
    }

    pub fn scheduler(&self) -> &Arc<RwLock<Scheduler>> {
        &self.scheduler
    }

    pub fn decision_count(&self) -> u64 {
        self.decisions.load(std::sync::atomic::Ordering::Relaxed)
    }

    /// Atualiza as métricas fuzzy observadas pelo loop (API/registry alimentam).
    pub fn set_metrics<F: FnMut(&mut FuzzyMetrics)>(&self, mut f: F) {
        let mut m = self.metrics.write();
        f(&mut m);
    }

    /// Porte fiel de `_collect_fuzzy_metrics` (Python `orchestrator/main.py`):
    /// coleta as 8 métricas de entrada dos decisores a partir dos caches do
    /// mesh (AgentRegistry + Tasks) e grava em `self.metrics`. Antes desta
    /// fiação (Rodada 8), `set_metrics` não tinha NENHUM chamador em produção
    /// e todo decisor adaptativo via `FuzzyMetrics::default()` (zeros
    /// constantes) em todo ciclo — degenerando em braço estático e
    /// invalidando qualquer comparação de braços (`--qos-manager`).
    ///
    /// Semântica idêntica ao Python, incluindo os defaults (0.5 em tudo,
    /// error_rate 0.1) quando não há dados:
    /// - `agent_load`   = slots ocupados / slots totais;
    /// - `recent_latency` = média das `ema_latency_ms` > 0, normalizada /1000, cap 1.0;
    /// - `error_rate`/`historical_confidence` = falhas/completadas sobre o total de ops;
    /// - `urgency`      = tasks ativas (PENDING|RUNNING) / total;
    /// - `streaming_need`/`estimated_complexity` = fração streaming e tamanho
    ///   médio de `messages_json` (/4000, cap 1.0) sobre as ATIVAS;
    /// - `deadline_pressure` = ativas com deadline estourado / total.
    pub fn refresh_metrics_from_mesh(&self) {
        let caches = self.dataspace.caches();
        let agents = caches.all_agents();
        let tasks = caches.all_tasks();

        let mut m = FuzzyMetrics {
            urgency: 0.5,
            deadline_pressure: 0.5,
            recent_latency: 0.5,
            agent_load: 0.5,
            error_rate: 0.1,
            historical_confidence: 0.5,
            estimated_complexity: 0.5,
            streaming_need: 0.5,
        };

        if !agents.is_empty() {
            let total_slots: u32 = agents.iter().map(|a| a.slots_total).sum();
            let busy_slots: u32 = agents.iter().map(|a| a.slots_busy).sum();
            m.agent_load = if total_slots > 0 {
                f64::from(busy_slots) / f64::from(total_slots)
            } else {
                0.0
            };

            let latencies: Vec<f64> = agents
                .iter()
                .map(|a| f64::from(a.ema_latency_ms))
                .filter(|&l| l > 0.0)
                .collect();
            if !latencies.is_empty() {
                let avg = latencies.iter().sum::<f64>() / latencies.len() as f64;
                m.recent_latency = (avg / 1000.0).min(1.0);
            }

            let completed: u64 = agents.iter().map(|a| u64::from(a.completed_total)).sum();
            let failed: u64 = agents.iter().map(|a| u64::from(a.failed_total)).sum();
            let ops = completed + failed;
            if ops > 0 {
                m.error_rate = failed as f64 / ops as f64;
                m.historical_confidence = completed as f64 / ops as f64;
            }
        }

        if !tasks.is_empty() {
            let now = now_ns();
            let total = tasks.len();
            let (mut active, mut overdue, mut streaming) = (0usize, 0usize, 0usize);
            let mut total_len = 0usize;

            for t in &tasks {
                // Igual ao Python: só PENDING (0) e RUNNING (2) são "ativas" —
                // ASSIGNED (1) fica de fora deliberadamente (paridade).
                if t.status == 0 || t.status == 2 {
                    active += 1;
                    if t.stream {
                        streaming += 1;
                    }
                    total_len += t.messages_json.len();
                    if t.deadline_ns > 0 && now > t.deadline_ns {
                        overdue += 1;
                    }
                }
            }

            m.urgency = (active as f64 / total as f64).min(1.0);
            if active > 0 {
                m.streaming_need = streaming as f64 / active as f64;
                let avg_len = total_len as f64 / active as f64;
                m.estimated_complexity = (avg_len / 4000.0).min(1.0);
            }
            m.deadline_pressure = (overdue as f64 / total as f64).min(1.0);
        }

        *self.metrics.write() = m;
    }

    /// Decide uma vez com as métricas correntes (expõe p/ testes e para o loop).
    pub fn decide_once(&self) -> QoSDecision {
        let m = *self.metrics.read();
        self.decider.decide(&qos_nfcm::decider::QoSMetrics {
            urgency: m.urgency,
            deadline_pressure: m.deadline_pressure,
            recent_latency: m.recent_latency,
            agent_load: m.agent_load,
            error_rate: m.error_rate,
            historical_confidence: m.historical_confidence,
            estimated_complexity: m.estimated_complexity,
            streaming_need: m.streaming_need,
        })
    }

    /// T-401: publica uma task no tópico `Tasks` com strength de CLIENTE (10) —
    /// os agentes (100) vencem a arbitragem ao clamar. (Se a API escrevesse com
    /// 200, nenhum agente conseguiria tomar a task.)
    ///
    /// Não alimenta `self.scheduler` aqui: nenhum caminho de produção chama
    /// `scheduler().pop()` (confirmado por busca — só o teste unitário do
    /// próprio `Scheduler` o exercita, construindo sua própria instância).
    /// Alimentá-lo custava um `RwLock::write().await` (serializando
    /// `publish_task` concorrentes entre si à toa) + um `task.clone()` em
    /// TODA requisição, sem nenhum consumidor real — achado de performance
    /// da Rodada 7. A struct/tipo `Scheduler` continua existindo (não é
    /// dead code do ponto de vista do tipo, só deste call site) caso um
    /// consumidor real apareça no futuro.
    pub async fn publish_task(&self, task: Task) -> Result<()> {
        self.api_tasks_writer
            .write(&task)
            .map_err(|e| dds_dataspace::api::DataSpaceError::Dds(e.to_string()))?;
        Ok(())
    }

    /// Alimenta os caches de Tasks/TaskOutput do orchestrator (visão do mesh).
    /// O orquestrador observa o espaço de dados — sem isto os caches ficam vazios.
    ///
    /// T-820-10: supervisão com recriação dos streams. Antes, `break` no
    /// primeiro `None` encerrava o feeder PARA SEMPE — `/sync` virava 504
    /// eterno, o reaper e as métricas congelavam, e só um log registrava o
    /// óbito. Agora: stream que termina é recriado após backoff crescente
    /// (1 s → dobra → teto 30 s; zera ao receber amostra saudável). A
    /// recriação é barata (reader novo sobre o mesmo `DataSpace`) e roda na
    /// MESMA task via `select!` com precondições — sem `tokio::spawn` filho,
    /// que ficaria órfão no abort externo (ver [`Self::spawn_registry_monitor`]).
    /// O sleep de backoff não bloqueia o OUTRO feeder: ele só desabilita a
    /// branch do stream morto (`if retry.is_none()`) até o prazo chegar.
    pub fn spawn_cache_feeders(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        const BASE_BACKOFF_MS: u64 = 1_000;
        const MAX_BACKOFF_MS: u64 = 30_000;
        let ds = Arc::clone(&self.dataspace);
        tokio::spawn(async move {
            let mut tasks_stream = Box::pin(ds.stream_tasks());
            let mut outputs_stream = Box::pin(ds.stream_task_outputs());
            let mut tasks_backoff_ms = BASE_BACKOFF_MS;
            let mut outputs_backoff_ms = BASE_BACKOFF_MS;
            // `Some(prazo)` = stream morto aguardando recriação (backoff).
            let mut tasks_retry_at: Option<tokio::time::Instant> = None;
            let mut outputs_retry_at: Option<tokio::time::Instant> = None;
            loop {
                let now = tokio::time::Instant::now();
                if tasks_retry_at.is_some_and(|at| now >= at) {
                    tracing::info!("stream_tasks recriado (supervisão T-820-10)");
                    tasks_stream = Box::pin(ds.stream_tasks());
                    tasks_retry_at = None;
                }
                if outputs_retry_at.is_some_and(|at| now >= at) {
                    tracing::info!("stream_task_outputs recriado (supervisão T-820-10)");
                    outputs_stream = Box::pin(ds.stream_task_outputs());
                    outputs_retry_at = None;
                }
                let next_retry = match (tasks_retry_at, outputs_retry_at) {
                    (Some(a), Some(b)) => Some(a.min(b)),
                    (Some(a), None) | (None, Some(a)) => Some(a),
                    (None, None) => None,
                };
                tokio::select! {
                    msg = tasks_stream.next(), if tasks_retry_at.is_none() => {
                        match msg {
                            Some(_) => tasks_backoff_ms = BASE_BACKOFF_MS, // amostra saudável
                            None => {
                                tracing::error!(
                                    backoff_ms = tasks_backoff_ms,
                                    "stream_tasks terminou — recriando com backoff (T-820-10)"
                                );
                                tasks_retry_at = Some(tokio::time::Instant::now()
                                    + Duration::from_millis(tasks_backoff_ms));
                                tasks_backoff_ms = (tasks_backoff_ms * 2).min(MAX_BACKOFF_MS);
                            }
                        }
                    }
                    msg = outputs_stream.next(), if outputs_retry_at.is_none() => {
                        match msg {
                            Some(_) => outputs_backoff_ms = BASE_BACKOFF_MS,
                            None => {
                                tracing::error!(
                                    backoff_ms = outputs_backoff_ms,
                                    "stream_task_outputs terminou — recriando com backoff (T-820-10)"
                                );
                                outputs_retry_at = Some(tokio::time::Instant::now()
                                    + Duration::from_millis(outputs_backoff_ms));
                                outputs_backoff_ms = (outputs_backoff_ms * 2).min(MAX_BACKOFF_MS);
                            }
                        }
                    }
                    _ = tokio::time::sleep_until(
                        next_retry.unwrap_or_else(tokio::time::Instant::now),
                    ), if next_retry.is_some() => {
                        // Acorda para recriar o(s) stream(s) cujo backoff venceu.
                    }
                }
            }
        })
    }

    /// T-403: monitor do registry + reapers.
    /// Assina AgentRegistry (alimenta o cache e o last_seen por agente) e, a cada
    /// `check_every`, executa DOIS reapers:
    /// - [`Self::reap_dead_agents`]: heartbeat parado há mais de `stale_after`
    ///   → agente morto; suas tasks ASSIGNED/RUNNING voltam para PENDING
    ///   (retry+1) e publica `QoS.Violation("liveliness_lost")`;
    /// - [`Self::reap_stalled_tasks`] (T-820-03): tasks ASSIGNED/RUNNING sem
    ///   transição além das janelas de progresso (30s/60s) voltam para PENDING
    ///   mesmo com o agente vivo — confirmação falso-negativa não prende mais
    ///   task (ver doc do método).
    ///
    /// Feeder do stream e reapers periódicos rodam na MESMA task via `select!`
    /// (não em uma task filha separada): um `tokio::spawn` interno ficaria
    /// órfão para sempre quando esta task fosse abortada externamente —
    /// `JoinHandle::abort` derruba esta future, mas `Drop` de um
    /// `JoinHandle` filho apenas o desanexa, não o cancela.
    pub fn spawn_registry_monitor(
        self: &Arc<Self>,
        stale_after: Duration,
        check_every: Duration,
    ) -> tokio::task::JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut stream = Box::pin(this.dataspace.stream_agent_states());
            let mut interval = tokio::time::interval(check_every);
            loop {
                tokio::select! {
                    maybe_state = stream.next() => {
                        match maybe_state {
                            Some(state) => {
                                this.last_seen.insert(state.agent_id.clone(), std::time::Instant::now());
                                this.registry.upsert((*state).clone());
                            }
                            None => {
                                tracing::warn!("stream AgentRegistry encerrado");
                                break;
                            }
                        }
                    }
                    _ = interval.tick() => {
                        this.reap_dead_agents(stale_after).await;
                        // T-820-03: reaper de progresso no MESMO tick — as
                        // janelas (30s/60s) são ordens de grandeza maiores
                        // que o ciclo típico (`check_every` = 2s), então o
                        // custo extra é uma varredura de cache por tick.
                        this.reap_stalled_tasks().await;
                        // T-820-03: republisher de reatribuições ainda não
                        // visíveis no mesh (relinquish de ownership pendente).
                        this.republishe_reatribuicoes_pendentes().await;
                    }
                }
            }
        })
    }

    /// Reatribui tasks de agentes mortos (ASSIGNED/RUNNING → PENDING, retry+1)
    /// e publica `QoS.Violation("liveliness_lost")` por agente — porte de
    /// `check_agent_liveliness` (`qos_monitor.py`), fundido aqui em vez de
    /// duplicado: o reaper já mantém o estado de "quem está vivo" via
    /// `last_seen`, então a violação usa a MESMA detecção, não uma segunda.
    async fn reap_dead_agents(&self, stale_after: Duration) {
        let now = std::time::Instant::now();
        // `HashSet`, não `Vec`: o `.contains()` abaixo roda por task em
        // `caches.all_tasks()` — O(1) por checagem em vez de O(agentes mortos).
        let dead: std::collections::HashSet<String> = self
            .last_seen
            .iter()
            .filter(|e| now.duration_since(*e.value()) > stale_after)
            .map(|e| e.key().clone())
            .collect();
        if dead.is_empty() {
            return;
        }
        tracing::warn!(agents = ?dead, "reaper: agentes mortos detectados (heartbeat parado)");

        for agent_id in &dead {
            self.publish_violation(
                "liveliness_lost",
                "AgentRegistry",
                "WRITER",
                agent_id,
                serde_json::json!({
                    "agent_id": agent_id,
                    "lease_duration_ms": stale_after.as_millis() as u64,
                }),
            )
            .await;
            // Remove de `last_seen` — sem isto, o mesmo agente já morto
            // continua batendo no filtro `duration_since(...) > stale_after`
            // em TODO ciclo seguinte (a cada `check_every`, tipicamente 2s),
            // republicando QoS.Violation("liveliness_lost") e o warn acima
            // indefinidamente até o agente reconectar (achado real: rodando
            // em produção por >2h contínuas contra um agente travado, ver
            // OPTIMIZATION_REPORT.md). Reconexão continua funcionando: a
            // linha 185 (`last_seen.insert(...)`) reinsere com timestamp
            // fresco assim que um novo `AgentRegistry` chegar na stream.
            self.last_seen.remove(agent_id);
        }

        let tasks = self.dataspace.caches().all_tasks();
        for t in tasks {
            if dead.contains(&t.assigned_agent) && (t.status == 1 || t.status == 2) {
                self.reassign_and_publish(&t, "liveliness_lost").await;
            }
        }
    }

    /// T-820-03: reatribui UMA task estagnada via [`crate::state_machine::
    /// reassign`] e republica SEM assumir ownership da instância.
    ///
    /// Por que `write_task_without_ownership` e não `write_task` (P0-1 do
    /// code review de 2026-10-04): o tópico `Tasks` usa
    /// `Ownership::Exclusive` com o writer do papel ORQUESTRATOR em strength
    /// 200 e os agentes em 100–163. A publicação pelo pool do orquestrador
    /// transferia o dono da instância para o orquestrador e os writes
    /// seguintes dos agentes (ASSIGNED do claim, RUNNING, DONE) perdiam a
    /// arbitragem do RHC para sempre — a task congelava em PENDING (o
    /// readback devolvia sempre PENDING; o cliente pendurava até o timeout).
    /// Publicando com strength de CLIENTE (10), o agente que reclamar vence
    /// a arbitragem normalmente; a proteção contra o write atrasado do
    /// agente "morto" original vem do `retry_count` no guard monotônico do
    /// cache (`cache::is_regression`) e no `confirm_ownership`.
    ///
    /// A transição passa por `state_machine::reassign` (não mutação manual):
    /// ganha o teto [`MAX_TASK_RETRIES`] → FAILED("MAX_RETRIES_EXCEEDED"),
    /// o guard de terminalidade e a renovação de `created_at_ns` (P0-2 —
    /// task re-emitida com nascimento antigo era rejeitada pelo filtro de
    /// idade de `claim::is_eligible` e ficava permanentemente in-claimável).
    /// `Ok(false)` (foi para FAILED) ainda é publicado: a terminalidade é
    /// justamente o que destrava o cliente.
    async fn reassign_and_publish(&self, task: &Task, cause: &str) {
        let mut reassigned = task.clone();
        match crate::state_machine::reassign(&mut reassigned, MAX_TASK_RETRIES) {
            Ok(requeued) => {
                match self
                    .dataspace
                    .write_task_without_ownership(reassigned.clone())
                    .await
                {
                    Ok(()) => {
                        if requeued {
                            tracing::info!(
                                task_id = %task.task_id,
                                retry = reassigned.retry_count,
                                cause,
                                "reaper: task reatribuída para PENDING (sem ownership)"
                            );
                            // T-820-03: o write em strength 10 pode ter sido
                            // REJEITADO pelo RHC (dono anterior — ASSIGNED do
                            // agente, strength 100+ — ainda registrado até a
                            // destruição do seu writer). Fica no republisher
                            // até o mesh confirmar o PENDING.
                            self.pending_reassignment.insert(
                                task.task_id.clone(),
                                (reassigned.clone(), std::time::Instant::now()),
                            );
                        } else {
                            tracing::warn!(
                                task_id = %task.task_id,
                                retries = reassigned.retry_count,
                                cause,
                                "reaper: teto de retries atingido — task publicada como FAILED(MAX_RETRIES_EXCEEDED)"
                            );
                        }
                    }
                    Err(e) => {
                        tracing::warn!(
                            task_id = %task.task_id,
                            error = %e,
                            "reaper: falha ao reatribuir"
                        );
                    }
                }
            }
            Err(e) => {
                // Status desconhecido no wire ou transição inválida: não toca
                // na task (a rede de segurança passa a ser o reaper de
                // progresso em ciclos futuros com estado de cache fresco).
                tracing::warn!(
                    task_id = %task.task_id,
                    status = task.status,
                    error = %e,
                    "reaper: transição de reatribuição inválida — task intocada"
                );
            }
        }
    }

    /// T-820-03: republisher das reatribuições ainda não visíveis no mesh.
    ///
    /// O write de reatribuição sai em strength de CLIENTE (10); sob Exclusive
    /// Ownership o RHC o descarta enquanto o writer do dono anterior seguir
    /// registrado — o ownership só é liberado pela DESTRUIÇÃO desse writer
    /// (`relinquish_ownership`; morte real do processo → lease do DDSI, ou
    /// delete gracioso → imediato). A cada tick, para cada entrada em
    /// `pending_reassignment`: se o mesh já reflete o PENDING (status 0,
    /// mesmo `retry_count`, sem dono) a entrada sai; se o TTL de 60 s estoura
    /// (ownership nunca liberado — p.ex. writer vazado), a entrada sai com
    /// warn; caso contrário, o PENDING é re-publicado.
    async fn republishe_reatribuicoes_pendentes(&self) {
        const REASSIGN_VISIBILITY_TTL: std::time::Duration = std::time::Duration::from_secs(60);
        let mut done: Vec<String> = Vec::new();
        for e in self.pending_reassignment.iter() {
            let (task_id, (pending, since)) = (e.key().clone(), e.value());
            let visible = match self.dataspace.read_task_mesh(&task_id) {
                Ok(Some(mesh)) => {
                    mesh.status == 0
                        && mesh.retry_count == pending.retry_count
                        && mesh.assigned_agent.is_empty()
                }
                _ => false,
            };
            if visible {
                done.push(task_id);
                continue;
            }
            if since.elapsed() > REASSIGN_VISIBILITY_TTL {
                tracing::warn!(
                    task_id = %task_id,
                    retry = pending.retry_count,
                    "republisher: reatribuição sem visibilidade após TTL — ownership do dono anterior nunca liberado?"
                );
                done.push(task_id);
                continue;
            }
            if let Err(err) = self
                .dataspace
                .write_task_without_ownership((*pending).clone())
                .await
            {
                tracing::debug!(task_id = %task_id, error = %err, "republisher: falha de write");
            }
        }
        for task_id in done {
            self.pending_reassignment.remove(&task_id);
        }
    }

    /// T-820-03: reaper de PROGRESSO — recolhe tasks ASSIGNED/RUNNING de
    /// agentes possivelmente VIVOS cujo estado não transiciona dentro das
    /// janelas [`ASSIGNED_STALL_AFTER`] / [`RUNNING_STALL_AFTER`] (medidas
    /// por `assigned_at_ns`/`started_at_ns`, sempre locais ao claim — ver
    /// `claim_task`). Complementa [`Self::reap_dead_agents`], que só cobre
    /// agentes com heartbeat parado: o caso "agente deu o claim como perdido
    /// mas o ASSIGNED dele venceu a arbitragem depois" (confirmação
    /// falso-negativa) deixa a task presa com o registry saudável — e a task
    /// nunca mais é elegível (`is_eligible` filtra status ≠ PENDING).
    async fn reap_stalled_tasks(&self) {
        let now = now_ns();
        for t in self.dataspace.caches().all_tasks() {
            let stalled = match t.status {
                s if s == orch_common::TaskStatus::Assigned as i32 => {
                    t.assigned_at_ns > 0
                        && now.saturating_sub(t.assigned_at_ns)
                            > ASSIGNED_STALL_AFTER.as_nanos() as u64
                }
                s if s == orch_common::TaskStatus::Running as i32 => {
                    t.started_at_ns > 0
                        && now.saturating_sub(t.started_at_ns)
                            > RUNNING_STALL_AFTER.as_nanos() as u64
                }
                _ => continue,
            };
            if stalled {
                tracing::info!(
                    task_id = %t.task_id,
                    status = t.status,
                    "reaper de progresso: task estagnada detectada"
                );
                self.reassign_and_publish(&t, "progresso_estagnado").await;
            }
        }
    }

    /// T-405/T-504: loop de controle com o decisor de QoS (`--qos-manager`).
    /// A cada `period`: coleta as métricas do mesh, decide o perfil, passa a
    /// decisão bruta pelo `StabilityController` (histerese/persistência/
    /// cooldown/fallback — §4.3/§4.6 do artigo, antes existente na crate mas
    /// nunca fiado aqui) e só então aplica os knobs online no writer de Tasks;
    /// cada decisão é tracejada (`qos_decision`, com perfil bruto E efetivo).
    ///
    /// Política de não convergência (fallback do artigo): se o decisor
    /// iterativo (FCM/NFCM) não convergiu (`!result.converged`), o ciclo
    /// mantém o perfil efetivo anterior — não aplica a decisão bruta.
    pub fn spawn_control_loop(self: &Arc<Self>, period: Duration) -> tokio::task::JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(period);
            let mut stability = qos_nfcm::stability::StabilityController::new(Default::default());
            loop {
                interval.tick().await;
                this.refresh_metrics_from_mesh();
                let result = this.decide_once();
                let raw_name = profile_name_of(&result.profile);
                let decision_n = this
                    .decisions
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    + 1;

                let effective_idx = if result.converged {
                    stability.update(result.profile.index(), result.confidence, result.runner_up)
                } else {
                    // Não convergiu: mantém o efetivo atual (ou o fallback
                    // Balanced se ainda não houve nenhuma decisão efetiva).
                    stability.current().unwrap_or(4)
                };
                let effective = qos_nfcm::QoSProfile::from_index(effective_idx);
                let profile_name = profile_name_of(&effective);

                match dds_contract::qos_profile(profile_name) {
                    Ok((_structural, knobs)) => {
                        if let Err(e) = this.dataspace.apply_tasks_knobs(&knobs) {
                            tracing::warn!(error = %e, "control loop: falha ao aplicar knobs");
                        }
                    }
                    Err(e) => {
                        tracing::warn!(profile = profile_name, error = %e, "control loop: perfil desconhecido");
                    }
                }

                tracing::info!(
                    decision = decision_n,
                    profile = profile_name,
                    raw_profile = raw_name,
                    score = result.confidence,
                    converged = result.converged,
                    explanation = %result.explanation,
                    "qos_decision"
                );

                this.maybe_publish_routing_profile(profile_name, result.confidence)
                    .await;
            }
        })
    }

    /// Porte de `_publish_fuzzy_routing_profile`: publica `QoS.RoutingProfile`
    /// só se `--fuzzy-routing` estiver ligado E o perfil mudou desde a última
    /// publicação (dedup — evita republicar o mesmo perfil a cada `period`).
    ///
    /// Fiel ao Python: a versão incrementa mesmo se a escrita falhar (não há
    /// rollback), mas o dedup (`last_routing_profile`) só avança em caso de
    /// sucesso — uma falha faz o próximo ciclo tentar de novo com versão nova.
    async fn maybe_publish_routing_profile(&self, profile_name: &str, confidence: f64) {
        if !self.fuzzy_routing {
            return;
        }
        if *self.last_routing_profile.lock() == profile_name {
            return;
        }

        let version = self
            .routing_version
            .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
            + 1;
        let profile =
            crate::qos_routing::build_routing_profile(profile_name, version, confidence, now_ns());
        let preferred = profile.preferred_agent_prefix.clone();
        match self.dataspace.write_qos_routing(profile).await {
            Ok(()) => {
                *self.last_routing_profile.lock() = profile_name.to_string();
                tracing::info!(
                    profile = profile_name,
                    version,
                    preferred = %preferred,
                    "Fuzzy Routing: perfil publicado"
                );
            }
            Err(e) => {
                tracing::warn!(profile = profile_name, error = %e, "falha ao publicar QoS.RoutingProfile");
            }
        }
    }

    /// Porte de `QoSMonitor._publish_violation`: incrementa os contadores
    /// (total + delta de janela) e publica `QoS.Violation`.
    async fn publish_violation(
        &self,
        violation_type: &str,
        topic_name: &str,
        entity_kind: &str,
        affected_entity: &str,
        details: serde_json::Value,
    ) {
        *self
            .qos_counters
            .entry(violation_type.to_string())
            .or_insert(0) += 1;
        *self
            .qos_window_deltas
            .entry(violation_type.to_string())
            .or_insert(0) += 1;

        let violation = crate::qos_monitor::build_violation(
            violation_type,
            topic_name,
            entity_kind,
            affected_entity,
            details,
            now_ns(),
        );
        if let Err(e) = self.dataspace.write_qos_violation(violation).await {
            tracing::warn!(violation_type, error = %e, "falha ao publicar QoS.Violation");
        }
    }

    /// Porte de `QoSMonitor.check_task_deadlines`: varre as tasks não-terminais
    /// do cache do mesh e publica `QoS.Violation("requested_deadline_missed")`
    /// na 1ª vez que cada `task_id` é visto passado do deadline (dedup via
    /// `reported_deadlines`, com o mesmo teto/eviction crua do Python).
    /// Observabilidade pura — não muda o estado da task (isso é um reaper
    /// diferente, que o Python tem em `TaskManager.reap_expired` e que ainda
    /// não existe no lado Rust; ver nota no relatório da fase).
    pub async fn check_task_deadlines(&self) -> usize {
        const REPORTED_DEADLINES_MAX: usize = 10_000;
        let now = now_ns();
        let mut new_count = 0usize;

        for t in self.dataspace.caches().all_tasks() {
            let task_id = t.task_id.clone();
            if task_id.is_empty() || self.reported_deadlines.contains(&task_id) {
                continue;
            }
            // TERMINAL_STATES do Python: DONE(3)/FAILED(4).
            if t.status == 3 || t.status == 4 {
                // Remove from reported_deadlines — task is done, no need to track.
                self.reported_deadlines.remove(&task_id);
                continue;
            }
            if t.created_at_ns == 0 || t.deadline_ns == 0 {
                continue;
            }
            if now > t.deadline_ns {
                let overdue_ms = (now - t.deadline_ns) as f64 / 1_000_000.0;
                let elapsed_ms = (now - t.created_at_ns) as f64 / 1_000_000.0;

                self.reported_deadlines.insert(task_id.clone());
                if self.reported_deadlines.len() > REPORTED_DEADLINES_MAX {
                    // Evict entries for terminal tasks instead of clearing all.
                    let terminal: Vec<String> = self
                        .reported_deadlines
                        .iter()
                        .filter(|id| {
                            self.dataspace
                                .caches()
                                .read_task(id)
                                .is_none_or(|t| t.status == 3 || t.status == 4)
                        })
                        .map(|id| id.clone())
                        .collect();
                    for id in terminal {
                        self.reported_deadlines.remove(&id);
                    }
                    // If still over max after terminal eviction, clear half.
                    if self.reported_deadlines.len() > REPORTED_DEADLINES_MAX {
                        let half = self.reported_deadlines.len() / 2;
                        let keys: Vec<String> = self
                            .reported_deadlines
                            .iter()
                            .take(half)
                            .map(|e| e.clone())
                            .collect();
                        for k in keys {
                            self.reported_deadlines.remove(&k);
                        }
                    }
                }
                new_count += 1;

                self.publish_violation(
                    "requested_deadline_missed",
                    "Tasks",
                    "READER",
                    &task_id,
                    serde_json::json!({
                        "task_id": task_id,
                        "overdue_ms": overdue_ms,
                        "elapsed_ms": elapsed_ms,
                    }),
                )
                .await;
            }
        }
        new_count
    }

    /// Porte de `QoSMonitor._publish_metrics`: publica um `QoS.Metric` por
    /// contador conhecido (total + delta desde a última chamada) e zera os
    /// deltas de janela.
    async fn publish_qos_metrics(&self) {
        let now = now_ns();
        let last = self
            .qos_last_publish_ns
            .swap(now, std::sync::atomic::Ordering::Relaxed);
        let window_ms = (now.saturating_sub(last) / 1_000_000) as i32;

        let names: Vec<String> = self.qos_counters.iter().map(|e| e.key().clone()).collect();
        for name in names {
            let value = self.qos_counters.get(&name).map(|v| *v).unwrap_or(0);
            let delta = self.qos_window_deltas.get(&name).map(|v| *v).unwrap_or(0);
            let metric = crate::qos_monitor::build_metric(&name, value, delta, window_ms, now);
            if let Err(e) = self.dataspace.write_qos_metric(metric).await {
                tracing::warn!(metric = %name, error = %e, "falha ao publicar QoS.Metric");
            }
        }
        self.qos_window_deltas.clear();
    }

    /// Porte de `QoSMonitor.run()`: a cada `period`, checa deadlines de tasks
    /// expiradas e publica `QoS.Metric` dos contadores. A detecção de
    /// liveliness perdida de agentes roda em `spawn_registry_monitor`
    /// (`reap_dead_agents`), que já publica `QoS.Violation` — não duplicada
    /// aqui (ver [`crate::qos_monitor`] para o porquê).
    pub fn spawn_qos_monitor(self: &Arc<Self>, period: Duration) -> tokio::task::JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(period);
            loop {
                interval.tick().await;
                this.check_task_deadlines().await;
                this.publish_qos_metrics().await;

                // Evict terminal tasks every cycle (30s max age) to bound memory.
                // Under sustained load (10K+ requests), caches grow fast.
                this.dataspace
                    .caches()
                    .evict_terminal_tasks(std::time::Duration::from_secs(30));
            }
        })
    }
}

/// Mapeia QoSProfile → nome canônico do perfil (`dds_contract::qos_profile`).
fn profile_name_of(p: &qos_nfcm::QoSProfile) -> &'static str {
    use qos_nfcm::QoSProfile::*;
    match p {
        Critical => "QoS_Critical",
        Failover => "QoS_Failover",
        StreamLike => "QoS_StreamLike",
        LowCost => "QoS_LowCost",
        Balanced => "QoS_Balanced",
    }
}
