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
//! Diferença decisiva vs `client::DdsClientDds::submit`: [`BenchmarkDriver::submit_observed`]
//! captura a **task terminal** do stream — é dela que vêm `assigned_agent` e
//! os componentes `t_*_ns` (E1). Sem isso os campos sairiam fabricados/zero.
//!
//! Contrato de medição (REQ/T-820-13):
//! - UM stream de status por corrida/loop (reader dedicado criado uma vez);
//!   um reader novo por request reprocessava o histórico TransientLocal
//!   KeepLast(50) dentro da janela medida (viés O(N²)).
//! - `warmup` é avaliado no envio (`started_ns`), nunca pós-conclusão; a
//!   corrida dura `warmup_s + duration_s` e a janela medida é `duration_s`
//!   (doc de `Scenario::duration_s`: "excluindo warmup").
//! - Padrão Closed itera a grade `concurrency` do cenário (uma fase por
//!   nível, sequencial), taggeando cada registro com `concurrency`.
//! - SIGINT: para de submeter, drena in-flight (melhor esforço) e flusha o
//!   JSONL antes de sair.

use std::path::PathBuf;
use std::sync::atomic::{AtomicBool, AtomicU32, AtomicU64, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

use client::dds_impl::DdsClientDds;
use client::{ClientConfig, ClientError, DdsClient};
use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_dataspace::api::DataSpaceApi;
use futures::{Stream, StreamExt};
use tokio::sync::Mutex;

use crate::metrics::{JsonlWriter, RequestRecord, RequestStatus};
use crate::regimes::WorkloadConfig;
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
    /// Duração da janela MEDIDA (s, excluindo warmup); 0 ⇒ usa a do cenário.
    pub duration_s: f64,
    pub seed: u64,
    pub out_dir: PathBuf,
    pub model_name: String,
    /// Braço de QoS anotado nos registros (ex.: "nfcm", "fixed_rules").
    pub qos_arm: String,
    /// Workers closed-loop. `0` (default) = usar a grade `concurrency`
    /// declarada no cenário (REQ/T-820-13); `N > 0` = sobrescreve com um
    /// único nível N (smoke/ad-hoc). Ignorado nos demais padrões.
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
            workers: 0,
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
    /// Níveis de concorrência efetivamente executados (padrão Closed; vazio
    /// nos demais). Um nível por fase, na ordem executada.
    pub concurrency_levels: Vec<u32>,
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
    /// Epoch (ns) do início da corrida — âncora da fronteira de warmup.
    run_started_ns: u64,
    /// Duração da janela MEDIDA (excluindo warmup).
    duration_s: f64,
    warmup_s: f64,
    /// Nível de concorrência da fase corrente (padrão Closed; 0 = fora dele).
    concurrency_level: AtomicU32,
    /// SIGINT: para de submeter e drena as requests in-flight (melhor esforço).
    cancelled: AtomicBool,
}

impl Shared {
    fn elapsed_s(&self) -> f64 {
        self.started.elapsed().as_secs_f64()
    }

    /// Deadline total = warmup + medição (REQ/T-820-13: a janela medida é
    /// `duration_s`, o warmup corre ANTES dela e é flagado nos registros).
    fn deadline_reached(&self) -> bool {
        self.elapsed_s() >= self.warmup_s + self.duration_s
    }

    fn cancelled(&self) -> bool {
        self.cancelled.load(Ordering::Relaxed)
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
    ///
    /// `warmup` é avaliado no momento do ENVIO (`started_ns` é capturado pelo
    /// chamador antes do submit — REQ/T-820-13), nunca pós-conclusão: a cauda
    /// do warmup não pode entrar no dataset medido por atraso de avaliação.
    fn base_record(
        &self,
        submitted: &Task,
        terminal: Option<&Task>,
        started_ns: u64,
        prompt_tokens: u32,
    ) -> RequestRecord {
        let submitted_elapsed_s =
            started_ns.saturating_sub(self.run_started_ns) as f64 / 1_000_000_000.0;
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
        let level = self.concurrency_level.load(Ordering::Relaxed);
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
            warmup: submitted_elapsed_s < self.warmup_s,
            // Campo aditivo (padrão Closed): nível de concorrência da fase.
            concurrency: (level > 0).then_some(level),
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
        // não pode perder metade do tempo em warmup). A corrida total dura
        // warmup_s + duration_s; a janela medida começa em warmup_s.
        let warmup_s = scenario.warmup_s.min(duration_s * 0.2);

        // run_id no nome do arquivo (REQ/T-820-13): re-runs não sobrescrevem
        // nem duplicam registros em append.
        let run_id = uuid::Uuid::new_v4().to_string();
        let out_file = cfg.out_dir.join(format!(
            "{}_{}_s{}_{}.jsonl",
            scenario.id.to_lowercase(),
            cfg.qos_arm,
            cfg.seed,
            &run_id[..8]
        ));
        let writer = JsonlWriter::create(&out_file)?;
        let run_started_ns = now_ns();

        Ok(Self {
            shared: Arc::new(Shared {
                client,
                factory,
                writer: Mutex::new(writer),
                counters: Counters::default(),
                cfg,
                scenario,
                started: Instant::now(),
                run_id,
                run_started_ns,
                duration_s,
                warmup_s,
                concurrency_level: AtomicU32::new(0),
                cancelled: AtomicBool::new(false),
            }),
        })
    }

    /// Executa a corrida conforme o padrão do cenário.
    ///
    /// Instala um handler de SIGINT: Ctrl+C para as submissões novas e drena
    /// as requests in-flight (melhor esforço, limitadas pelo timeout) antes
    /// do flush do JSONL — nada gravado se perde no encerramento
    /// (REQ/T-820-13).
    pub async fn run(self) -> Result<RunSummary, BenchError> {
        // Handler de Ctrl+C: sinaliza cancelamento; os loops consultam o flag
        // e param de submeter. In-flight continuam até o terminal/timeout
        // (dreno best-effort).
        let cancel_task = {
            let shared = Arc::clone(&self.shared);
            tokio::spawn(async move {
                if tokio::signal::ctrl_c().await.is_ok() {
                    tracing::warn!(
                        "SIGINT recebido — parando submissões; drenando requests in-flight"
                    );
                    shared.cancelled.store(true, Ordering::Relaxed);
                }
            })
        };

        let mut concurrency_levels = Vec::new();
        match self.shared.scenario.pattern.clone() {
            WorkloadPattern::Open { regime } => self.open_loop(false, regime).await,
            WorkloadPattern::Streaming { regime } => self.open_loop(true, regime).await,
            WorkloadPattern::Closed { concurrency } => {
                // Grade declarada no cenário (REQ/T-820-13): uma fase por
                // nível, sequencial; `--workers N` sobrescreve com 1 nível.
                let levels: Vec<u32> = if self.shared.cfg.workers > 0 {
                    vec![self.shared.cfg.workers]
                } else {
                    concurrency.to_vec()
                };
                concurrency_levels = levels.clone();
                self.closed_loop(&levels).await;
            }
            WorkloadPattern::Priority {
                background_rps,
                n_injections,
            } => self.priority_loop(background_rps, n_injections).await,
        }

        cancel_task.abort();

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
            concurrency_levels,
        })
    }

    /// Submete e aguarda o terminal, CAPTURANDO a task final do stream
    /// (com `assigned_agent` e `t_*_ns`). Mesma máquina de estados de
    /// `client::DdsClientDds::submit`, mas retorna a task terminal.
    ///
    /// REQ/T-820-13: o stream de status é criado UMA vez por corrida/loop e
    /// reutilizado entre requests (`status_stream`) — criar um reader novo
    /// por request reprocessava o histórico TransientLocal KeepLast(50)
    /// dentro da janela medida (viés O(N²) na corrida).
    async fn submit_observed<S>(
        &self,
        task: Task,
        status_stream: &mut S,
    ) -> (Result<(), ClientError>, Option<Task>)
    where
        S: Stream<Item = Arc<Task>> + Unpin,
    {
        submit_observed_shared(&self.shared, task, status_stream).await
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
    async fn one_request<S>(
        &self,
        gen: &mut crate::generator::WorkloadGenerator,
        priority: i32,
        status_stream: &mut S,
    ) where
        S: Stream<Item = Arc<Task>> + Unpin,
    {
        let prompt = gen.generate_prompt();
        let prompt_tokens = prompt.split_whitespace().count() as u32;
        let max_tokens = gen.config().max_tokens_response;
        let task = self.shared.make_task(&prompt, priority, false, max_tokens);
        let started_ns = now_ns();
        let t0 = Instant::now();
        let (outcome, terminal) = self.submit_observed(task.clone(), status_stream).await;
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
        let mut ended_final = false;

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
                        ended_final = true;
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
        // Stream que termina sem `is_final` NÃO é Ok (REQ/T-820-13): sem o
        // chunk terminal a resposta está truncada — registra erro honesto.
        rec.status = failed.unwrap_or(if ended_final {
            RequestStatus::Ok
        } else {
            rec.error_message = Some("stream_end_sem_final".into());
            RequestStatus::Error
        });
        self.shared.record(rec).await;
    }

    /// Open-loop Poisson (porte de `run_workload`): inter-arrival exponencial
    /// (com burst), requests sequenciais até `warmup_s + duration_s`. O
    /// `regime` vem do dispatch em `run` — sem segundo match, sem
    /// `unreachable!`.
    ///
    /// UM stream de status para a corrida inteira (REQ/T-820-13 — sem reader
    /// novo por request).
    async fn open_loop(&self, stream: bool, regime: WorkloadConfig) {
        let mut gen = crate::generator::WorkloadGenerator::new(regime, self.shared.cfg.seed);
        let mut status_stream = Box::pin(self.shared.client.dataspace().stream_tasks());

        while !self.shared.deadline_reached() && !self.shared.cancelled() {
            let ia = gen.next_inter_arrival(self.shared.elapsed_s());
            tokio::time::sleep(Duration::from_secs_f64(ia)).await;
            if self.shared.deadline_reached() || self.shared.cancelled() {
                break;
            }
            self.shared
                .counters
                .submitted
                .fetch_add(1, Ordering::Relaxed);
            if stream {
                self.one_stream(&mut gen).await;
            } else {
                self.one_request(&mut gen, PRIORITY_NORMAL, &mut status_stream)
                    .await;
            }
        }
    }

    /// Closed-loop: uma fase por nível de concorrência da grade do cenário
    /// (REQ/T-820-13 — E2=[10,50], OP1=[1,10,25,50]); dentro da fase, N
    /// workers sequenciais request→resposta (sem sleep) — vazão sob
    /// concorrência limitada (porte de run_op1_reduced).
    ///
    /// Janela: a fase 1 inclui o warmup da corrida (registros com
    /// `warmup=true` são descartados na análise); as fases seguintes são
    /// 100% medidas, com `duration_s/níveis` cada.
    async fn closed_loop(&self, levels: &[u32]) {
        let n_levels = levels.len().max(1) as u32;
        let measured_phase_s = self.shared.duration_s / f64::from(n_levels);
        let mut phase_end =
            self.shared.started + Duration::from_secs_f64(self.shared.warmup_s + measured_phase_s);
        for (idx, &level) in levels.iter().enumerate() {
            if self.shared.deadline_reached() || self.shared.cancelled() {
                break;
            }
            // Fase corrente: registros taggeados com o nível (campo aditivo).
            self.shared
                .concurrency_level
                .store(level, Ordering::Relaxed);
            tracing::info!(
                level,
                phase = idx + 1,
                of = levels.len(),
                phase_s = measured_phase_s,
                "closed-loop: iniciando fase de concorrência"
            );
            self.run_workers(level, phase_end).await;
            // Próxima fase: measured_phase_s após o fim desta.
            phase_end += Duration::from_secs_f64(measured_phase_s);
        }
        self.shared.concurrency_level.store(0, Ordering::Relaxed);
    }

    /// Roda `workers` tasks concorrentes até `phase_end` (ou deadline/cancel
    /// global), cada worker com stream de status próprio (um por worker, não
    /// por request).
    async fn run_workers(&self, workers: u32, phase_end: Instant) {
        let workers = workers.max(1);
        let mut handles = Vec::with_capacity(workers as usize);
        for w in 0..workers {
            let shared = Arc::clone(&self.shared);
            let seed = shared.cfg.seed.wrapping_add(u64::from(w) + 1);
            let phase_end = phase_end;
            handles.push(tokio::spawn(async move {
                let mut gen = crate::generator::WorkloadGenerator::new(crate::regimes::LEVE, seed);
                let mut status_stream = Box::pin(shared.client.dataspace().stream_tasks());
                while Instant::now() < phase_end
                    && !shared.deadline_reached()
                    && !shared.cancelled()
                {
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
    /// `max_tokens=1` para medir fila, não inferência). Fundo e injeções têm
    /// UM stream de status cada (REQ/T-820-13 — sem reader novo por request).
    async fn priority_loop(&self, background_rps: f64, n_injections: u32) {
        // Fundo NORMAL.
        let shared = Arc::clone(&self.shared);
        let bg = tokio::spawn(async move {
            let mut gen =
                crate::generator::WorkloadGenerator::new(crate::regimes::LEVE, shared.cfg.seed);
            let mut rng = Rng::new(shared.cfg.seed ^ 0x0B6C_41A0);
            let mut status_stream = Box::pin(shared.client.dataspace().stream_tasks());
            while !shared.deadline_reached() && !shared.cancelled() {
                let ia = rng.exponential(background_rps);
                tokio::time::sleep(Duration::from_secs_f64(ia)).await;
                if shared.deadline_reached() || shared.cancelled() {
                    break;
                }
                shared.counters.submitted.fetch_add(1, Ordering::Relaxed);
                let prompt = gen.generate_prompt();
                let prompt_tokens = prompt.split_whitespace().count() as u32;
                let task = shared.make_task(&prompt, PRIORITY_NORMAL, false, 1);
                let started_ns = now_ns();
                let t0 = Instant::now();
                let (outcome, terminal) =
                    submit_observed_shared(&shared, task.clone(), &mut status_stream).await;
                let mut rec =
                    shared.base_record(&task, terminal.as_ref(), started_ns, prompt_tokens);
                rec.load_level = "background_normal".into();
                BenchmarkDriver::finish_record(&mut rec, &outcome, t0);
                shared.record(rec).await;
            }
        });

        // Injeções HIGH (mesma duração medida do fundo: o espaçamento cobre
        // apenas `duration_s` — o deadline global já inclui o warmup).
        let inject_interval = (self.shared.duration_s / f64::from(n_injections)).max(0.1);
        let mut gen = crate::generator::WorkloadGenerator::new(
            crate::regimes::LEVE,
            self.shared.cfg.seed.wrapping_add(1),
        );
        let mut status_stream = Box::pin(self.shared.client.dataspace().stream_tasks());
        for _ in 0..n_injections {
            tokio::time::sleep(Duration::from_secs_f64(inject_interval)).await;
            if self.shared.deadline_reached() || self.shared.cancelled() {
                break;
            }
            self.shared
                .counters
                .submitted
                .fetch_add(1, Ordering::Relaxed);
            let prompt = gen.generate_prompt();
            let prompt_tokens = prompt.split_whitespace().count() as u32;
            let task = self.shared.make_task(&prompt, PRIORITY_HIGH, false, 1);
            let started_ns = now_ns();
            let t0 = Instant::now();
            let (outcome, terminal) = self.submit_observed(task.clone(), &mut status_stream).await;
            let mut rec =
                self.shared
                    .base_record(&task, terminal.as_ref(), started_ns, prompt_tokens);
            rec.load_level = "injection_high".into();
            Self::finish_record(&mut rec, &outcome, t0);
            self.shared.record(rec).await;
        }

        bg.abort();
    }
}

/// Núcleo de `submit_observed` para uso em tasks spawned (onde não há
/// `&self` do driver): recebe o stream de status JÁ CRIADO pelo chamador —
/// um por worker/loop, nunca um reader novo por request (REQ/T-820-13).
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
