//! Runtime DDS do orchestrator (T-401/T-403/T-405).
//!
//! - `publish_task`: API → Task no tópico `Tasks` (agentes claim — data-centric).
//! - `spawn_registry_monitor`: assina AgentRegistry + liveliness; agente morto
//!   reatribui suas tasks não-terminais para PENDING (strength 200) e publica
//!   `QoS.Violation("liveliness_lost")`.
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
}

impl OrchestratorDds {
    /// Sobe o runtime (orquestrador = papel strength 200 para reaper/failover).
    /// O decisor de QoS vem do `--qos-manager` (T-504).
    pub fn new(domain_id: u32, decider: Arc<dyn QosDecider>) -> Result<Self> {
        let dataspace = Arc::new(DataSpace::new(domain_id, DataSpace::STRENGTH_ORCHESTRATOR)?);
        let api_qos = dds_dataspace::qos::profiles::tasks(Some(DataSpace::STRENGTH_CLIENT))?;
        let api_tasks_writer = dataspace.tasks_writer_with(&api_qos);
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
        })
    }

    /// Liga a publicação de `QoS.RoutingProfile` (porte de `--fuzzy-routing`).
    pub fn with_fuzzy_routing(mut self, enabled: bool) -> Self {
        self.fuzzy_routing = enabled;
        self
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
    pub async fn publish_task(&self, task: Task) -> Result<()> {
        self.scheduler.write().await.push(task.clone());
        self.api_tasks_writer
            .write(&task)
            .map_err(|e| dds_dataspace::api::DataSpaceError::Dds(e.to_string()))?;
        Ok(())
    }

    /// Alimenta os caches de Tasks/TaskOutput do orchestrator (visão do mesh).
    /// O orquestrador observa o espaço de dados — sem isto os caches ficam vazios.
    pub fn spawn_cache_feeders(self: &Arc<Self>) -> tokio::task::JoinHandle<()> {
        let ds = Arc::clone(&self.dataspace);
        tokio::spawn(async move {
            let mut t = Box::pin(ds.stream_tasks());
            let mut o = Box::pin(ds.stream_task_outputs());
            loop {
                tokio::select! {
                    _ = t.next() => {}
                    _ = o.next() => {}
                }
            }
        })
    }

    /// T-403: monitor do registry + reaper.
    /// Assina AgentRegistry (alimenta o cache e o last_seen por agente) e, a cada
    /// `check_every`, marca como mortos os agentes com heartbeat parado há mais de
    /// `stale_after` — suas tasks ASSIGNED/RUNNING voltam para PENDING (retry+1).
    ///
    /// Feeder do stream e reaper periódico rodam na MESMA task via `select!`
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
                let mut reassigned = (*t).clone();
                reassigned.status = 0; // PENDING
                reassigned.assigned_agent = String::new();
                reassigned.retry_count += 1;
                if let Err(e) = self.dataspace.write_task(reassigned.clone()).await {
                    tracing::warn!(task_id = %t.task_id, error = %e, "reaper: falha ao reatribuir");
                } else {
                    tracing::info!(task_id = %t.task_id, retry = reassigned.retry_count, "reaper: task reatribuída para PENDING");
                }
            }
        }
    }

    /// T-405/T-504: loop de controle com o decisor de QoS (`--qos-manager`).
    /// A cada `period`: decide o perfil com as métricas correntes, aplica os
    /// knobs online no writer de Tasks e traceja a decisão (`qos_decision`).
    pub fn spawn_control_loop(self: &Arc<Self>, period: Duration) -> tokio::task::JoinHandle<()> {
        let this = Arc::clone(self);
        tokio::spawn(async move {
            let mut interval = tokio::time::interval(period);
            loop {
                interval.tick().await;
                let result = this.decide_once();
                let profile_name = profile_name_of(&result.profile);
                let decision_n = this
                    .decisions
                    .fetch_add(1, std::sync::atomic::Ordering::Relaxed)
                    + 1;

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
                    confidence = result.confidence,
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
                    self.reported_deadlines.clear();
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
