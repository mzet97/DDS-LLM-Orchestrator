//! Driver de benchmark (feature `dds`) — publica tasks via DDS e grava
//! `RequestRecord`s em JSONL por cenário/braço.
//!
//! Portes:
//! - `real_workload_driver.py` → open-loop Poisson ([`BenchmarkDriver::open_loop`]);
//! - `run_op1_reduced.py`/E2 → closed-loop com N workers ([`closed_loop`]);
//! - `E3_priority.py` → fundo NORMAL + injeções HIGH ([`priority_loop`]);
//! - `run_e5_ttft.py` → streaming com TTFC/ICL ([`one_stream`]) — renomeado no Gate A.
//!
//! A análise estatística (Friedman/mixed models, índice de Jain) NÃO está
//! aqui — permanece nos scripts Python (`benchmarks/qualificacao/analysis/`).
//!
//! Diferença decisiva vs `client::DdsClientDds::submit`: [`submit_observed`]
//! captura a **task terminal** do stream — é dela que vêm `assigned_agent` e
//! os componentes `t_*_ns` (E1). Sem isso os campos sairiam fabricados/zero.

use std::path::PathBuf;
use std::sync::atomic::{AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use client::dds_impl::DdsClientDds;
use client::{ClientConfig, ClientError, DdsClient};
use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_dataspace::api::DataSpaceApi;
use futures::{Stream, StreamExt};
use tokio::sync::Mutex;

use crate::metrics::{JsonlWriter, RequestRecord, RequestStatus};
use crate::rng::Rng;
use crate::scenarios::{Scenario, WorkloadPattern};

/// `TaskPriority` do `models.py` (IntEnum no IDL).
pub const PRIORITY_LOW: i32 = 1;
pub const PRIORITY_NORMAL: i32 = 5;
pub const PRIORITY_HIGH: i32 = 10;

/// Timeout default por request (30 s — paridade com `wait_for_output`).
const DEFAULT_TIMEOUT_MS: u64 = 30_000;
/// `TaskStatus` no IDL.
const STATUS_DONE: i32 = 3;
const STATUS_FAILED: i32 = 4;

#[derive(Debug, thiserror::Error)]
pub enum BenchError {
    #[error("cliente DDS: {0}")]
    Client(#[from] ClientError),
    #[error("io: {0}")]
    Io(#[from] std::io::Error),
    #[error("cenário inválido: {0}")]
    InvalidScenario(String),
}

/// Configuração de uma corrida de benchmark.
#[derive(Debug, Clone)]
pub struct DriverConfig {
    pub domain: u32,
    /// Duração da medição (s); 0 ⇒ usa a do cenário.
    pub duration_s: f64,
    pub seed: u64,
    pub out_dir: PathBuf,
    pub model_name: String,
    /// Braço de QoS anotado nos registros (ex.: "nfcm", "fixed_rules").
    pub qos_arm: String,
    /// Workers closed-loop (padrão Closed; ignorado nos demais).
    pub workers: u32,
    pub timeout_ms: u64,
}

impl Default for DriverConfig {
    fn default() -> Self {
        Self {
            domain: 0,
            duration_s: 0.0,
            seed: 42,
            out_dir: PathBuf::from("./bench_out"),
            model_name: "local".into(),
            qos_arm: "nfcm".into(),
            workers: 10,
            timeout_ms: DEFAULT_TIMEOUT_MS,
        }
    }
}

/// Resumo de uma corrida.
#[derive(Debug, Clone)]
pub struct RunSummary {
    pub submitted: u64,
    pub ok: u64,
    pub errors: u64,
    pub timeouts: u64,
    pub elapsed_s: f64,
    pub out_file: PathBuf,
}

/// Contadores compartilhados entre workers.
#[derive(Default)]
struct Counters {
    submitted: AtomicU64,
    ok: AtomicU64,
    errors: AtomicU64,
    timeouts: AtomicU64,
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

/// Contexto compartilhado por todos os loops do driver.
struct Shared {
    client: DdsClientDds,
    factory: DdsClient,
    writer: Mutex<JsonlWriter>,
    counters: Counters,
    cfg: DriverConfig,
    scenario: Scenario,
    started: Instant,
    run_id: String,
    duration_s: f64,
    warmup_s: f64,
}

impl Shared {
    fn elapsed_s(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    fn deadline_reached(&self) -> bool {
        self.elapsed_s() >= self.duration_s
    }

    /// Cria a task (payload idêntico ao Python:
    /// `[{"role": "user", "content": prompt}]`).
    fn make_task(&self, prompt: &str, priority: i32, stream: bool, max_tokens: u32) -> Task {
        let messages_json = serde_json::json!([{"role": "user", "content": prompt}]).to_string();
        let mut task =
            self.factory
                .create_task(&self.cfg.model_name, &messages_json, priority, stream);
        task.max_tokens = max_tokens;
        task
    }

    async fn record(&self, rec: RequestRecord) {
        match rec.status {
            RequestStatus::Ok => {
                self.counters.ok.fetch_add(1, Ordering::Relaxed);
            }
            RequestStatus::Timeout => {
                self.counters.timeouts.fetch_add(1, Ordering::Relaxed);
            }
            _ => {
                self.counters.errors.fetch_add(1, Ordering::Relaxed);
            }
        }
        let mut w = self.writer.lock().await;
        if let Err(e) = w.append(&rec) {
            tracing::warn!(error = %e, "falha ao gravar JSONL");
        }
    }

    /// Monta o registro. `terminal` é a task final observada no stream (com
    /// `assigned_agent` e `t_*_ns` preenchidos pelo agente/orquestrador);
    /// `submitted_*` são os dados da task enviada (fallback honesto).
    fn base_record(
        &self,
        submitted: &Task,
        terminal: Option<&Task>,
        started_ns: u64,
        prompt_tokens: u32,
    ) -> RequestRecord {
        let elapsed = self.elapsed_s();
        let (load_level, target_rps) = match &self.scenario.pattern {
            WorkloadPattern::Open { regime } | WorkloadPattern::Streaming { regime } => {
                (regime.name.to_string(), regime.lambda_rps)
            }
            WorkloadPattern::Priority { background_rps, .. } => {
                ("background".to_string(), *background_rps)
            }
            WorkloadPattern::Closed { .. } => ("closed".to_string(), 0.0),
        };
        let t = terminal.unwrap_or(submitted);
        RequestRecord {
            trace_id: submitted.task_id.clone(),
            experiment_id: format!("{}_rust", self.scenario.id.to_lowercase()),
            run_id: self.run_id.clone(),
            test_id: self.scenario.id.to_lowercase(),
            protocol: "dds".into(),
            agent_id: t.assigned_agent.clone(),
            model_name: submitted.model_name.clone(),
            qos_arm: self.cfg.qos_arm.clone(),
            prompt_input_tokens_estimated: prompt_tokens,
            max_output_tokens: submitted.max_tokens,
            started_at_ns: started_ns,
            finished_at_ns: now_ns(),
            status: RequestStatus::Ok,
            error_message: None,
            load_level,
            target_rps,
            replication_idx: 0,
            warmup: elapsed < self.warmup_s,
            latency_ms: 0.0,
            t_serialization_ns: none_if_zero(t.t_serialization_ns),
            t_transport_send_ns: none_if_zero(t.t_transport_send_ns),
            t_agent_queue_ns: none_if_zero(t.t_agent_queue_ns),
            t_inference_ns: none_if_zero(t.t_inference_ns),
            t_transport_return_ns: none_if_zero(t.t_transport_return_ns),
            t_deserialization_ns: none_if_zero(t.t_deserialization_ns),
            ttfc_ms: None,
            icl_mean_ms: None,
            n_chunks: None,
        }
    }
}

fn none_if_zero(v: u64) -> Option<u64> {
    if v == 0 {
        None
    } else {
        Some(v)
    }
}

/// Driver de benchmark.
pub struct BenchmarkDriver {
    shared: Arc<Shared>,
}

impl BenchmarkDriver {
    /// Cria o driver: UM participante DDS (strength cliente=10, via `client`).
    pub fn new(cfg: DriverConfig, scenario: Scenario) -> Result<Self, BenchError> {
        scenario.validate().map_err(BenchError::InvalidScenario)?;
        let client_cfg = ClientConfig {
            client_id: format!("bench-{}", uuid::Uuid::new_v4()),
            dds_domain: cfg.domain,
            timeout_ms: cfg.timeout_ms,
        };
        let client = DdsClientDds::new(client_cfg.clone())?;
        let factory = DdsClient::new(client_cfg);

        let duration_s = if cfg.duration_s > 0.0 {
            cfg.duration_s
        } else {
            scenario.duration_s
        };
        // Warmup do cenário, limitado a 20% da duração efetiva (smoke curto
        // não pode perder metade do tempo em warmup).
        let warmup_s = scenario.warmup_s.min(duration_s * 0.2);

        let out_file = cfg.out_dir.join(format!(
            "{}_{}_s{}.jsonl",
            scenario.id.to_lowercase(),
            cfg.qos_arm,
            cfg.seed
        ));
        let writer = JsonlWriter::create(&out_file)?;

        Ok(Self {
            shared: Arc::new(Shared {
                client,
                factory,
                writer: Mutex::new(writer),
                counters: Counters::default(),
                cfg,
                scenario,
                started: Instant::now(),
                run_id: uuid::Uuid::new_v4().to_string(),
                duration_s,
                warmup_s,
            }),
        })
    }

    /// Executa a corrida conforme o padrão do cenário.
    pub async fn run(self) -> Result<RunSummary, BenchError> {
        match self.shared.scenario.pattern.clone() {
            WorkloadPattern::Open { .. } => self.open_loop(false).await,
            WorkloadPattern::Streaming { .. } => self.open_loop(true).await,
            WorkloadPattern::Closed { .. } => self.closed_loop().await,
            WorkloadPattern::Priority {
                background_rps,
                n_injections,
            } => self.priority_loop(background_rps, n_injections).await,
        }

        let mut w = self.shared.writer.lock().await;
        w.flush()?;
        let out_file = w.path().to_path_buf();
        drop(w);

        Ok(RunSummary {
            submitted: self.shared.counters.submitted.load(Ordering::Relaxed),
            ok: self.shared.counters.ok.load(Ordering::Relaxed),
            errors: self.shared.counters.errors.load(Ordering::Relaxed),
            timeouts: self.shared.counters.timeouts.load(Ordering::Relaxed),
            elapsed_s: self.shared.elapsed_s(),
            out_file,
        })
    }

    /// Submete e aguarda o terminal, CAPTURANDO a task final do stream
    /// (com `assigned_agent` e `t_*_ns`). Mesma máquina de estados de
    /// `client::DdsClientDds::submit`, mas retorna a task terminal.
    async fn submit_observed(&self, task: Task) -> (Result<(), ClientError>, Option<Task>) {
        let task_id = task.task_id.clone();
        let dataspace = self.shared.client.dataspace();
        if let Err(e) = dataspace.write_task(task).await {
            return (Err(ClientError::DdsError(e.to_string())), None);
        }

        let deadline = Instant::now() + Duration::from_millis(self.shared.cfg.timeout_ms);
        let mut status_stream = Box::pin(dataspace.stream_tasks());
        loop {
            let remaining = deadline.saturating_duration_since(Instant::now());
            if remaining.is_zero() {
                return (Err(ClientError::Timeout(task_id)), None);
            }
            tokio::select! {
                st = status_stream.next() => {
                    let Some(t) = st else { return (Err(ClientError::Timeout(task_id)), None); };
                    if t.task_id != task_id { continue; }
                    match t.status {
                        STATUS_DONE => return (Ok(()), Some((*t).clone())),
                        STATUS_FAILED => {
                            return (Err(ClientError::TaskFailed(t.finish_reason.clone())), Some((*t).clone()));
                        }
                        _ => continue,
                    }
                }
                _ = tokio::time::sleep(remaining) => {
                    return (Err(ClientError::Timeout(task_id)), None);
                }
            }
        }
    }

    /// Preenche status/latência/erro do registro a partir do outcome.
    fn finish_record(rec: &mut RequestRecord, outcome: &Result<(), ClientError>, t0: Instant) {
        rec.latency_ms = t0.elapsed().as_secs_f64() * 1000.0;
        match outcome {
            Ok(()) => rec.status = RequestStatus::Ok,
            Err(ClientError::Timeout(_)) => rec.status = RequestStatus::Timeout,
            Err(e) => {
                rec.status = RequestStatus::Error;
                rec.error_message = Some(e.to_string());
            }
        }
    }

    /// Executa UMA request não-streaming e grava o registro.
    async fn one_request(&self, gen: &mut crate::generator::WorkloadGenerator, priority: i32) {
        let prompt = gen.generate_prompt();
        let prompt_tokens = prompt.split_whitespace().count() as u32;
        let max_tokens = gen.config().max_tokens_response;
        let task = self.shared.make_task(&prompt, priority, false, max_tokens);
        let started_ns = now_ns();
        let t0 = Instant::now();
        let (outcome, terminal) = self.submit_observed(task.clone()).await;
        let mut rec = self
            .shared
            .base_record(&task, terminal.as_ref(), started_ns, prompt_tokens);
        Self::finish_record(&mut rec, &outcome, t0);
        self.shared.record(rec).await;
    }

    /// Uma request em streaming (E5): mede TTFC/ICL (chunks visíveis) até `is_final` —
    /// nomenclatura canônica: docs/tracking/DICIONARIO_METRICAS.md (Gate A).
    async fn one_stream(&self, gen: &mut crate::generator::WorkloadGenerator) {
        let prompt = gen.generate_prompt();
        let prompt_tokens = prompt.split_whitespace().count() as u32;
        let max_tokens = gen.config().max_tokens_response;
        let task = self
            .shared
            .make_task(&prompt, PRIORITY_NORMAL, true, max_tokens);
        let started_ns = now_ns();
        let t0 = Instant::now();
        let mut rec = self
            .shared
            .base_record(&task, None, started_ns, prompt_tokens);

        let mut stream = self.shared.client.submit_stream(task);
        let mut first_chunk: Option<Instant> = None;
        let mut last_chunk: Option<Instant> = None;
        let mut itl_sum = Duration::ZERO;
        let mut n_chunks = 0u32;
        let mut failed: Option<RequestStatus> = None;

        while let Some(item) = stream.next().await {
            match item {
                Ok(chunk) => {
                    let now = Instant::now();
                    if first_chunk.is_none() {
                        first_chunk = Some(now);
                    }
                    if let Some(prev) = last_chunk {
                        itl_sum += now - prev;
                    }
                    last_chunk = Some(now);
                    n_chunks += 1;
                    if chunk.is_final {
                        break;
                    }
                }
                Err(ClientError::Timeout(_)) => {
                    failed = Some(RequestStatus::Timeout);
                    break;
                }
                Err(e) => {
                    failed = Some(RequestStatus::Error);
                    rec.error_message = Some(e.to_string());
                    break;
                }
            }
        }

        rec.latency_ms = t0.elapsed().as_secs_f64() * 1000.0;
        rec.ttfc_ms = first_chunk.map(|t| (t - t0).as_secs_f64() * 1000.0);
        if n_chunks > 1 {
            rec.icl_mean_ms = Some(itl_sum.as_secs_f64() * 1000.0 / (n_chunks - 1) as f64);
        }
        rec.n_chunks = Some(n_chunks);
        rec.status = failed.unwrap_or(RequestStatus::Ok);
        self.shared.record(rec).await;
    }

    /// Open-loop Poisson (porte de `run_workload`): inter-arrival exponencial
    /// (com burst), requests sequenciais até `duration_s`.
    async fn open_loop(&self, stream: bool) {
        let regime = match &self.shared.scenario.pattern {
            WorkloadPattern::Open { regime } | WorkloadPattern::Streaming { regime } => *regime,
            _ => unreachable!("open_loop só roda padrões Open/Streaming"),
        };
        let mut gen = crate::generator::WorkloadGenerator::new(regime, self.shared.cfg.seed);

        while !self.shared.deadline_reached() {
            let ia = gen.next_inter_arrival(self.shared.elapsed_s());
            tokio::time::sleep(Duration::from_secs_f64(ia)).await;
            if self.shared.deadline_reached() {
                break;
            }
            self.shared
                .counters
                .submitted
                .fetch_add(1, Ordering::Relaxed);
            if stream {
                self.one_stream(&mut gen).await;
            } else {
                self.one_request(&mut gen, PRIORITY_NORMAL).await;
            }
        }
    }

    /// Closed-loop: N workers, cada um sequencial request→resposta (sem
    /// sleep) — vazão sob concorrência limitada (porte de run_op1_reduced).
    async fn closed_loop(&self) {
        let workers = self.shared.cfg.workers.max(1);
        let mut handles = Vec::with_capacity(workers as usize);
        for w in 0..workers {
            let shared = Arc::clone(&self.shared);
            let seed = shared.cfg.seed.wrapping_add(u64::from(w) + 1);
            handles.push(tokio::spawn(async move {
                let mut gen = crate::generator::WorkloadGenerator::new(crate::regimes::LEVE, seed);
                let mut status_stream = Box::pin(shared.client.dataspace().stream_tasks());
                while !shared.deadline_reached() {
                    shared.counters.submitted.fetch_add(1, Ordering::Relaxed);
                    let prompt = gen.generate_prompt();
                    let prompt_tokens = prompt.split_whitespace().count() as u32;
                    let task = shared.make_task(&prompt, PRIORITY_NORMAL, false, 50);
                    let started_ns = now_ns();
                    let t0 = Instant::now();
                    let (outcome, terminal) =
                        submit_observed_shared(&shared, task.clone(), &mut status_stream).await;
                    let mut rec =
                        shared.base_record(&task, terminal.as_ref(), started_ns, prompt_tokens);
                    BenchmarkDriver::finish_record(&mut rec, &outcome, t0);
                    shared.record(rec).await;
                }
            }));
        }
        for h in handles {
            let _ = h.await;
        }
    }

    /// Priority (E3/OP4): fundo NORMAL em open-loop + injeções HIGH espaçadas
    /// (porte de `E3_priority.py`: injeção a cada `duracao/n`, mínimo 0,1 s;
    /// `max_tokens=1` para medir fila, não inferência).
    async fn priority_loop(&self, background_rps: f64, n_injections: u32) {
        // Fundo NORMAL.
        let shared = Arc::clone(&self.shared);
        let bg = tokio::spawn(async move {
            let mut gen =
                crate::generator::WorkloadGenerator::new(crate::regimes::LEVE, shared.cfg.seed);
            let mut rng = Rng::new(shared.cfg.seed ^ 0x0B6C_41A0);
            let mut status_stream = Box::pin(shared.client.dataspace().stream_tasks());
            while !shared.deadline_reached() {
                let ia = rng.exponential(background_rps);
                tokio::time::sleep(Duration::from_secs_f64(ia)).await;
                if shared.deadline_reached() {
                    break;
                }
                shared.counters.submitted.fetch_add(1, Ordering::Relaxed);
                let prompt = gen.generate_prompt();
                let task = shared.make_task(&prompt, PRIORITY_NORMAL, false, 1);
                let started_ns = now_ns();
                let t0 = Instant::now();
                let (outcome, terminal) =
                    submit_observed_shared(&shared, task.clone(), &mut status_stream).await;
                let mut rec = shared.base_record(&task, terminal.as_ref(), started_ns, 0);
                rec.load_level = "background_normal".into();
                BenchmarkDriver::finish_record(&mut rec, &outcome, t0);
                shared.record(rec).await;
            }
        });

        // Injeções HIGH.
        let inject_interval = (self.shared.duration_s / f64::from(n_injections)).max(0.1);
        let mut gen = crate::generator::WorkloadGenerator::new(
            crate::regimes::LEVE,
            self.shared.cfg.seed.wrapping_add(1),
        );
        for _ in 0..n_injections {
            tokio::time::sleep(Duration::from_secs_f64(inject_interval)).await;
            if self.shared.deadline_reached() {
                break;
            }
            self.shared
                .counters
                .submitted
                .fetch_add(1, Ordering::Relaxed);
            let prompt = gen.generate_prompt();
            let task = self.shared.make_task(&prompt, PRIORITY_HIGH, false, 1);
            let started_ns = now_ns();
            let t0 = Instant::now();
            let (outcome, terminal) = self.submit_observed(task.clone()).await;
            let mut rec = self
                .shared
                .base_record(&task, terminal.as_ref(), started_ns, 0);
            rec.load_level = "injection_high".into();
            Self::finish_record(&mut rec, &outcome, t0);
            self.shared.record(rec).await;
        }

        bg.abort();
    }
}

/// Versão free-function de `submit_observed` para uso em tasks spawned
/// (onde não há `&self` do driver).
async fn submit_observed_shared<S>(
    shared: &Shared,
    task: Task,
    status_stream: &mut S,
) -> (Result<(), ClientError>, Option<Task>)
where
    S: Stream<Item = Arc<Task>> + Unpin,
{
    let task_id = task.task_id.clone();
    let dataspace = shared.client.dataspace();
    if let Err(e) = dataspace.write_task(task).await {
        return (Err(ClientError::DdsError(e.to_string())), None);
    }
    let deadline = Instant::now() + Duration::from_millis(shared.cfg.timeout_ms);
    loop {
        let remaining = deadline.saturating_duration_since(Instant::now());
        if remaining.is_zero() {
            return (Err(ClientError::Timeout(task_id)), None);
        }
        tokio::select! {
            st = status_stream.next() => {
                let Some(t) = st else { return (Err(ClientError::Timeout(task_id)), None); };
                if t.task_id != task_id { continue; }
                match t.status {
                    STATUS_DONE => return (Ok(()), Some((*t).clone())),
                    STATUS_FAILED => {
                        return (Err(ClientError::TaskFailed(t.finish_reason.clone())), Some((*t).clone()));
                    }
                    _ => continue,
                }
            }
            _ = tokio::time::sleep(remaining) => {
                return (Err(ClientError::Timeout(task_id)), None);
            }
        }
    }
}

/// Braços de QoS disponíveis para varrer em E4 (baselines incluídos).
/// Os decisores vivem em `qos-nfcm` (porte com paridade verificada — WF-7).
pub fn available_arms() -> [&'static str; 9] {
    [
        "static",
        "zadeh",
        "fcm",
        "fcm-dhl",
        "nfcm",
        "fixed_rules",
        "mamdani",
        "ucb1",
        "sw_ucb",
    ]
}
