//! # orch-common
//!
//! Tipos, config, métricas e instrumentação compartilhados. Substitui
//! `src/orchestrator/common/` (Python).
//!
//! ## Ganhos de Rust já aplicáveis aqui
//! - **`tracing`** estruturado (JSON) em vez de `logging` — mesmo trace de decisão
//!   de QoS (`qos_decision`) que instrumentei no Python, agora sem custo de GIL.
//! - **Contadores atômicos / `parking_lot`** para métricas concorrentes — o bug
//!   C3 (RTTTracker/ErrorTracker sem lock, corrompendo latência) desaparece: em
//!   Rust as métricas são `AtomicU64`/`Mutex` sem GIL, corretas por construção.

/// Erro de conversão de um `i32` cru do wire para um enum tipado — o valor
/// não corresponde a nenhuma variante conhecida.
#[derive(Debug, Clone, Copy, PartialEq, Eq, thiserror::Error)]
#[error("valor i32 desconhecido para {enum_name}: {value}")]
pub struct UnknownEnumValue {
    pub enum_name: &'static str,
    pub value: i32,
}

/// Estados de tarefa (espelha `TaskStatus` do IDL — `Task.status`, campo cru
/// `long` no wire; ver `OrchestratorV4.idl`). Discriminantes explícitos para
/// que `as i32` continue produzindo exatamente o valor que já trafega hoje —
/// esta é uma view tipada por cima do campo cru, não uma mudança de wire
/// format (P3 do `OPTIMIZATION_PLAN.md`: consumidores que comparam o `i32`
/// cru continuam funcionando sem alteração).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum TaskStatus {
    Pending = 0,
    Assigned = 1,
    Running = 2,
    Done = 3,
    Failed = 4,
}

impl TryFrom<i32> for TaskStatus {
    type Error = UnknownEnumValue;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Assigned),
            2 => Ok(Self::Running),
            3 => Ok(Self::Done),
            4 => Ok(Self::Failed),
            _ => Err(UnknownEnumValue {
                enum_name: "TaskStatus",
                value: v,
            }),
        }
    }
}

impl From<TaskStatus> for i32 {
    fn from(v: TaskStatus) -> i32 {
        v as i32
    }
}

/// Prioridade de tarefa (`Task.priority`). **Os valores NÃO são a numeração
/// sequencial 0/1/2 que `OrchestratorV4.idl`'s `enum TaskPriority` implicaria
/// por ordem de declaração** — são 1/5/10, os valores realmente usados em
/// todo o código (ver `benchmarks::driver::{PRIORITY_LOW,PRIORITY_NORMAL,
/// PRIORITY_HIGH}` e o campo `priority` de `Task` em toda a base). O IDL
/// declara o enum só como documentação; o campo do wire é `long` cru.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum TaskPriority {
    Low = 1,
    Normal = 5,
    High = 10,
}

impl TryFrom<i32> for TaskPriority {
    type Error = UnknownEnumValue;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            1 => Ok(Self::Low),
            5 => Ok(Self::Normal),
            10 => Ok(Self::High),
            _ => Err(UnknownEnumValue {
                enum_name: "TaskPriority",
                value: v,
            }),
        }
    }
}

impl From<TaskPriority> for i32 {
    fn from(v: TaskPriority) -> i32 {
        v as i32
    }
}

/// Especialização de modelo (`Task.model_required`). Os discriminantes são
/// os mesmos do `ModelSpecialization` canônico em `OrchestratorV4.idl`
/// (REQ-708), incluindo o agente Whisper existente (`Transcription = 3`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ModelSpecialization {
    Text = 0,
    Vision = 1,
    Embedding = 2,
    Transcription = 3,
}

impl TryFrom<i32> for ModelSpecialization {
    type Error = UnknownEnumValue;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Text),
            1 => Ok(Self::Vision),
            2 => Ok(Self::Embedding),
            3 => Ok(Self::Transcription),
            _ => Err(UnknownEnumValue {
                enum_name: "ModelSpecialization",
                value: v,
            }),
        }
    }
}

impl From<ModelSpecialization> for i32 {
    fn from(v: ModelSpecialization) -> i32 {
        v as i32
    }
}

impl ModelSpecialization {
    /// Returns whether this agent specialization can serve `required`.
    /// Text agents are the generic fallback; vision also accepts text tasks.
    pub const fn can_serve(self, required: Self) -> bool {
        matches!(
            (self, required),
            (Self::Text, _)
                | (Self::Vision, Self::Text | Self::Vision)
                | (Self::Embedding, Self::Embedding)
                | (Self::Transcription, Self::Transcription)
        )
    }
}

/// Saúde do agente (`AgentState.health`) — bate com a ordem declarada em
/// `OrchestratorV4.idl`'s `enum AgentHealth` (`AH_OFFLINE=0, AH_DEGRADED=1,
/// AH_HEALTHY=2`), já usada assim em `heartbeat.rs`/testes desta sessão.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum AgentHealth {
    Offline = 0,
    Degraded = 1,
    Healthy = 2,
}

impl TryFrom<i32> for AgentHealth {
    type Error = UnknownEnumValue;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Offline),
            1 => Ok(Self::Degraded),
            2 => Ok(Self::Healthy),
            _ => Err(UnknownEnumValue {
                enum_name: "AgentHealth",
                value: v,
            }),
        }
    }
}

impl From<AgentHealth> for i32 {
    fn from(v: AgentHealth) -> i32 {
        v as i32
    }
}

/// Motivo de finalização (`TaskOutput.finish_reason`/`Task.finish_reason`
/// nos pontos que usam o campo int32 unificado — ver `dds_types.h` no lado
/// C++, e o comentário "campo unificado e int32" em `agent/src/dds.rs`).
/// Bate com `OrchestratorV4.idl`'s `enum FinishReason` por ordem de
/// declaração (`FR_NONE=0, FR_COMPLETION=1, FR_LENGTH=2, FR_TIMEOUT=3,
/// FR_ERROR=4`).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum FinishReason {
    None = 0,
    Completion = 1,
    Length = 2,
    Timeout = 3,
    Error = 4,
}

impl TryFrom<i32> for FinishReason {
    type Error = UnknownEnumValue;
    fn try_from(v: i32) -> Result<Self, UnknownEnumValue> {
        match v {
            0 => Ok(Self::None),
            1 => Ok(Self::Completion),
            2 => Ok(Self::Length),
            3 => Ok(Self::Timeout),
            4 => Ok(FinishReason::Error),
            _ => Err(UnknownEnumValue {
                enum_name: "FinishReason",
                value: v,
            }),
        }
    }
}

impl From<FinishReason> for i32 {
    fn from(v: FinishReason) -> i32 {
        v as i32
    }
}

/// Componente do sistema (`SystemMetric.component_type`) — bate com
/// `OrchestratorV4.idl`'s `enum ComponentType` por ordem de declaração
/// (`CT_ORCHESTRATOR=0, CT_AGENT=1, CT_LLAMA_SERVER=2, CT_CLIENT=3`). Sem
/// evidência direta de uso divergente encontrada nesta sessão — ao
/// contrário de `TaskPriority`/`ModelSpecialization` acima, cuja numeração
/// real diverge do IDL.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ComponentType {
    Orchestrator = 0,
    Agent = 1,
    LlamaServer = 2,
    Client = 3,
}

impl TryFrom<i32> for ComponentType {
    type Error = UnknownEnumValue;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Orchestrator),
            1 => Ok(Self::Agent),
            2 => Ok(Self::LlamaServer),
            3 => Ok(Self::Client),
            _ => Err(UnknownEnumValue {
                enum_name: "ComponentType",
                value: v,
            }),
        }
    }
}

impl From<ComponentType> for i32 {
    fn from(v: ComponentType) -> i32 {
        v as i32
    }
}

/// Nível de segurança (`ContextSnapshot.security_level`/
/// `ToolCallRequest.security_level`/`LLMInferenceRequest.security_level`) —
/// mapeado pelo comentário já presente no IDL/C++
/// (`dds/idl/OrchestratorDDS.idl`: "SecurityLevel enum (0=PUBLIC,
/// 1=INTERNAL, etc.)"). Só 2 níveis confirmados por comentário direto; os
/// demais ("etc.") não têm evidência de valor exato nesta sessão — usar
/// `TryFrom` (não um `From` infalível) reflete essa incerteza honestamente.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum SecurityLevel {
    Public = 0,
    Internal = 1,
}

impl TryFrom<i32> for SecurityLevel {
    type Error = UnknownEnumValue;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Public),
            1 => Ok(Self::Internal),
            _ => Err(UnknownEnumValue {
                enum_name: "SecurityLevel",
                value: v,
            }),
        }
    }
}

impl From<SecurityLevel> for i32 {
    fn from(v: SecurityLevel) -> i32 {
        v as i32
    }
}

/// Status de uma chamada de ferramenta (`ToolCallRequest.status`). Sem
/// enum declarado no IDL e sem evidência de valores usados nesta sessão —
/// modelado com o padrão mínimo óbvio (pendente/concluído/falhou) até haver
/// confirmação de um consumidor real; **valores especulativos, revisar
/// antes de depender deles**.
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ToolCallStatus {
    Pending = 0,
    Completed = 1,
    Failed = 2,
}

impl TryFrom<i32> for ToolCallStatus {
    type Error = UnknownEnumValue;
    fn try_from(v: i32) -> Result<Self, Self::Error> {
        match v {
            0 => Ok(Self::Pending),
            1 => Ok(Self::Completed),
            2 => Ok(Self::Failed),
            _ => Err(UnknownEnumValue {
                enum_name: "ToolCallStatus",
                value: v,
            }),
        }
    }
}

impl From<ToolCallStatus> for i32 {
    fn from(v: ToolCallStatus) -> i32 {
        v as i32
    }
}

/// Vetor de métricas de estado do sistema (as 8 entradas do NFCM).
/// Coletado pelo control loop; alimenta o decisor de QoS e o trace de treino.
#[derive(Debug, Clone, Copy, Default)]
pub struct FuzzyMetrics {
    pub urgency: f64,
    pub deadline_pressure: f64,
    pub recent_latency: f64,
    pub agent_load: f64,
    pub error_rate: f64,
    pub historical_confidence: f64,
    pub estimated_complexity: f64,
    pub streaming_need: f64,
}

impl FuzzyMetrics {
    /// Ordem canônica das métricas (igual ao `METRICS` do qos-nfcm).
    pub fn to_array(&self) -> [f64; 8] {
        [
            self.urgency,
            self.deadline_pressure,
            self.recent_latency,
            self.agent_load,
            self.error_rate,
            self.historical_confidence,
            self.estimated_complexity,
            self.streaming_need,
        ]
    }
}

/// Instrumentação: spans de latência, contadores de RTT, rastreamento de erros.
/// Substitui `common/instrumentation.py` com contadores atômicos (sem lock).
pub mod instrumentation {
    use std::sync::atomic::{AtomicU64, Ordering};
    use std::time::Instant;

    /// Span de latência para decomposição T1-T6 (serialização, transporte, fila,
    /// inferência, transporte de volta, deserialização).
    #[derive(Debug)]
    pub struct LatencySpan {
        pub name: &'static str,
        start: Instant,
    }

    impl LatencySpan {
        /// Inicia um span.
        pub fn start(name: &'static str) -> Self {
            Self {
                name,
                start: Instant::now(),
            }
        }

        /// Finaliza o span e retorna a duração em nanossegundos.
        pub fn finish(self) -> u64 {
            self.start.elapsed().as_nanos() as u64
        }
    }

    /// Contador de RTT com média exponencial (EMA).
    #[derive(Debug)]
    pub struct RttTracker {
        count: AtomicU64,
        total_ns: AtomicU64,
        ema_ns: AtomicU64,
    }

    impl Default for RttTracker {
        fn default() -> Self {
            Self::new()
        }
    }

    impl RttTracker {
        pub fn new() -> Self {
            Self {
                count: AtomicU64::new(0),
                total_ns: AtomicU64::new(0),
                ema_ns: AtomicU64::new(0),
            }
        }

        /// Registra uma medição de RTT.
        pub fn record(&self, rtt_ns: u64) {
            self.count.fetch_add(1, Ordering::Relaxed);
            self.total_ns.fetch_add(rtt_ns, Ordering::Relaxed);
            // EMA: new = 0.9 * old + 0.1 * observed
            let old = self.ema_ns.load(Ordering::Relaxed);
            let new_val = if old == 0 {
                rtt_ns
            } else {
                (old as f64 * 0.9 + rtt_ns as f64 * 0.1) as u64
            };
            self.ema_ns.store(new_val, Ordering::Relaxed);
        }

        /// Retorna RTT médio em nanossegundos.
        pub fn mean_ns(&self) -> u64 {
            let count = self.count.load(Ordering::Relaxed);
            self.total_ns
                .load(Ordering::Relaxed)
                .checked_div(count)
                .unwrap_or(0)
        }

        /// Retorna EMA do RTT em nanossegundos.
        pub fn ema_ns(&self) -> u64 {
            self.ema_ns.load(Ordering::Relaxed)
        }

        /// Retorna número de medições.
        pub fn count(&self) -> u64 {
            self.count.load(Ordering::Relaxed)
        }

        /// Reseta o tracker.
        pub fn reset(&self) {
            self.count.store(0, Ordering::Relaxed);
            self.total_ns.store(0, Ordering::Relaxed);
            self.ema_ns.store(0, Ordering::Relaxed);
        }
    }

    /// Contador de erros por categoria.
    #[derive(Debug)]
    pub struct ErrorCounter {
        total: AtomicU64,
        by_category: [AtomicU64; 8], // até 8 categorias
    }

    impl Default for ErrorCounter {
        fn default() -> Self {
            Self::new()
        }
    }

    impl ErrorCounter {
        pub fn new() -> Self {
            Self {
                total: AtomicU64::new(0),
                by_category: std::array::from_fn(|_| AtomicU64::new(0)),
            }
        }

        /// Registra um erro em uma categoria (0-7).
        pub fn record(&self, category: usize) {
            self.total.fetch_add(1, Ordering::Relaxed);
            if category < self.by_category.len() {
                self.by_category[category].fetch_add(1, Ordering::Relaxed);
            }
        }

        /// Retorna total de erros.
        pub fn total(&self) -> u64 {
            self.total.load(Ordering::Relaxed)
        }

        /// Retorna erros por categoria.
        pub fn by_category(&self, category: usize) -> u64 {
            if category < self.by_category.len() {
                self.by_category[category].load(Ordering::Relaxed)
            } else {
                0
            }
        }
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn task_status_variants() {
        assert_ne!(TaskStatus::Pending, TaskStatus::Running);
        assert_ne!(TaskStatus::Done, TaskStatus::Failed);
    }

    #[test]
    fn model_specialization_discriminants_and_routing_are_canonical() {
        assert_eq!(i32::from(ModelSpecialization::Text), 0);
        assert_eq!(i32::from(ModelSpecialization::Vision), 1);
        assert_eq!(i32::from(ModelSpecialization::Embedding), 2);
        assert_eq!(i32::from(ModelSpecialization::Transcription), 3);
        assert!(ModelSpecialization::Text.can_serve(ModelSpecialization::Vision));
        assert!(ModelSpecialization::Text.can_serve(ModelSpecialization::Embedding));
        assert!(ModelSpecialization::Text.can_serve(ModelSpecialization::Transcription));
        assert!(ModelSpecialization::Vision.can_serve(ModelSpecialization::Text));
        assert!(!ModelSpecialization::Vision.can_serve(ModelSpecialization::Embedding));
        assert!(ModelSpecialization::Transcription.can_serve(ModelSpecialization::Transcription));
        assert!(ModelSpecialization::try_from(-1).is_err());
        assert!(ModelSpecialization::try_from(4).is_err());
    }

    #[test]
    fn fuzzy_metrics_to_array() {
        let m = FuzzyMetrics {
            urgency: 0.1,
            deadline_pressure: 0.2,
            recent_latency: 0.3,
            agent_load: 0.4,
            error_rate: 0.5,
            historical_confidence: 0.6,
            estimated_complexity: 0.7,
            streaming_need: 0.8,
        };
        let arr = m.to_array();
        assert_eq!(arr, [0.1, 0.2, 0.3, 0.4, 0.5, 0.6, 0.7, 0.8]);
    }

    #[test]
    fn rtt_tracker_ema() {
        let rt = instrumentation::RttTracker::new();
        rt.record(1000);
        assert_eq!(rt.ema_ns(), 1000);
        rt.record(2000);
        // EMA: 0.9 * 1000 + 0.1 * 2000 = 1100
        assert_eq!(rt.ema_ns(), 1100);
        assert_eq!(rt.count(), 2);
    }

    #[test]
    fn error_counter_basic() {
        let ec = instrumentation::ErrorCounter::new();
        ec.record(0);
        ec.record(0);
        ec.record(3);
        assert_eq!(ec.total(), 3);
        assert_eq!(ec.by_category(0), 2);
        assert_eq!(ec.by_category(3), 1);
        assert_eq!(ec.by_category(7), 0);
    }

    #[test]
    fn latency_span_measures_time() {
        let span = instrumentation::LatencySpan::start("test");
        std::thread::sleep(std::time::Duration::from_millis(10));
        let ns = span.finish();
        assert!(ns >= 10_000_000); // at least 10ms
    }
}
