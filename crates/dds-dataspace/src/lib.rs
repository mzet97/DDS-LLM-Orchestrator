//! # dds-dataspace
//!
//! A camada DDS de coordenação. Substitui `src/orchestrator/dds_backend/`
//! (~3,4k LOC Python) — o componente onde o GIL mais dói. É o **2º alvo** da
//! migração (após o agente).
//!
//! ## Como Rust remove os gargalos que mapeei no relatório
//! | Gargalo Python | Solução Rust nesta crate |
//! |---|---|
//! | Poll loop 20ms + churn por amostra | **WaitSet compartilhado** (`dispatch::SharedWaitSet`, Fase 5/T-617) + streams assíncronas: acorda por evento, zero polling, 1 thread de espera por `DataSpace` em vez de 1 por stream |
//! | Alocação por amostra (`dds_to_task`) | **Zero-copy loans** (`take_loan`) — sample sem cópia no hot path |
//! | Thread ÚNICA de escrita (serialização) | **N writers + `crossbeam-channel` MPMC**; sem GIL, escrita realmente paralela |
//! | Caches (dict + RLock global) | **`dashmap`** (sharded, lock-free) — leituras de agente não serializam com escrita de task |
//! | Guardas anti-regressão (C1) | Ownership por papel nativa + tipos imutáveis (`Arc<Task>`) — a corrida estrutural some |
//! | Liveliness por polling | **Listener nativo** (`on_liveliness_changed`) SEM o deadlock de GIL do Python |
//!
//! Compile com `--features dds` para usar o runtime DDS real.

pub mod api;
pub mod cache;
#[cfg(feature = "dds")]
pub mod dispatch;
pub mod in_memory;
pub mod qos;

use dashmap::DashMap;
use std::sync::Arc;

/// Cache de tópico concorrente e lock-free (substitui dict + RLock global).
pub type TopicCache<T> = Arc<DashMap<String, T>>;

#[cfg(feature = "security")]
pub use cyclonedds::SecurityConfig;
#[cfg(feature = "dds")]
use cyclonedds::{DataReader, DataWriter, DomainParticipant, Publisher, Subscriber, Topic};
#[cfg(feature = "dds")]
use dds_contract::generated::dds_llm_orchestrator::{
    AgentState, ContextSnapshot, ContextUpdate, DiscoveryEvent, ExecutionTraceEvent, QoSMetric,
    QoSRoutingProfile, QoSViolation, SecurityPolicySnapshot, SecurityPolicyUpdate, SystemMetric,
    Task, TaskOutput, ToolCallRequest,
};
#[cfg(feature = "dds")]
use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult, ServerStatus,
};
#[cfg(feature = "dds")]
use dds_contract::topics;

/// DataSpace real: participant/publisher/subscriber, tópicos canônicos com o QoS
/// que casa com a malha Python (ver `qos::profiles`), readers/writers por tópico.
///
/// T-302: ciclo de vida (sobe/derruba limpo). T-303..T-306 constroem a API async
/// (`DataSpaceApi`) por cima.
#[cfg(feature = "dds")]
pub struct DataSpace {
    // Ordem de drop: filhos (writers/readers) antes dos pais (topics/pub/sub/participant).

    // Tópicos originais (3)
    // Pool de writers de `Tasks` (ver `task_writer_for` para o porquê de mais
    // de um).
    tasks_writers: Vec<DataWriter<Task>>,
    agents_writer: DataWriter<AgentState>,
    outputs_writer: DataWriter<TaskOutput>,
    // `tasks_reader` é usado por `read_task_mesh`/confirmação de ownership
    // (leitura pontual do RHC arbitrado) — distinto dos readers 'static
    // dedicados que cada `stream_*` cria por chamada (ver nota abaixo).
    tasks_reader: DataReader<Task>,
    tasks_topic: Arc<Topic<Task>>,
    agents_topic: Arc<Topic<AgentState>>,
    outputs_topic: Arc<Topic<TaskOutput>>,

    // Runtime telemetry (2)
    system_metrics_writer: DataWriter<SystemMetric>,
    server_status_writer: DataWriter<ServerStatus>,
    system_metrics_topic: Arc<Topic<SystemMetric>>,
    server_status_topic: Arc<Topic<ServerStatus>>,

    // Tópicos LLM (3)
    llm_request_writer: DataWriter<LLMInferenceRequest>,
    llm_result_writer: DataWriter<LLMInferenceResult>,
    llm_error_writer: DataWriter<LLMInferenceError>,
    llm_request_topic: Arc<Topic<LLMInferenceRequest>>,
    llm_result_topic: Arc<Topic<LLMInferenceResult>>,
    llm_error_topic: Arc<Topic<LLMInferenceError>>,

    // Tópicos Context (2)
    context_snapshot_writer: DataWriter<ContextSnapshot>,
    context_update_writer: DataWriter<ContextUpdate>,
    context_snapshot_topic: Arc<Topic<ContextSnapshot>>,
    context_update_topic: Arc<Topic<ContextUpdate>>,

    // Tópicos ToolCall (1)
    tool_call_writer: DataWriter<ToolCallRequest>,
    tool_call_topic: Arc<Topic<ToolCallRequest>>,
    tool_call_reader: DataReader<ToolCallRequest>,

    // Tópicos ExecutionTrace (1)
    execution_trace_writer: DataWriter<ExecutionTraceEvent>,
    execution_trace_topic: Arc<Topic<ExecutionTraceEvent>>,

    // Tópicos Security (2)
    security_snapshot_writer: DataWriter<SecurityPolicySnapshot>,
    security_update_writer: DataWriter<SecurityPolicyUpdate>,
    security_snapshot_topic: Arc<Topic<SecurityPolicySnapshot>>,
    security_update_topic: Arc<Topic<SecurityPolicyUpdate>>,

    // Tópicos QoS (3)
    qos_routing_writer: DataWriter<QoSRoutingProfile>,
    qos_metric_writer: DataWriter<QoSMetric>,
    qos_violation_writer: DataWriter<QoSViolation>,
    discovery_event_writer: DataWriter<DiscoveryEvent>,
    qos_routing_topic: Arc<Topic<QoSRoutingProfile>>,
    qos_metric_topic: Arc<Topic<QoSMetric>>,
    qos_violation_topic: Arc<Topic<QoSViolation>>,
    discovery_event_topic: Arc<Topic<DiscoveryEvent>>,

    // Infraestrutura compartilhada
    publisher: Arc<Publisher>,
    subscriber: Arc<Subscriber>,
    // Nunca lido diretamente: mantido apenas para manter o participant (e,
    // por RAII, toda a árvore de entidades DDS abaixo dele) vivo pelo
    // lifetime do DataSpace. Derrubá-lo cedo destruiria publisher/subscriber/
    // topics/writers/readers.
    #[allow(dead_code)]
    participant: DomainParticipant,
    ownership_strength: i32,
    caches: Arc<TopicCaches>,
    /// WaitSet único compartilhado por todos os `stream_*()` (Fase 5/T-617) —
    /// ver `dispatch.rs`. `Arc` porque cada stream clona uma referência para
    /// se registrar e ficar viva independente do lifetime de `&self`.
    shared_waitset: Arc<dispatch::SharedWaitSet>,
}

/// Constrói o pool de writers de `Tasks` para um `ownership_strength` dado —
/// compartilhado por `DataSpace::new()` e `DataSpace::new_writer_pool()` para
/// que os DOIS caminhos de escrita de `Tasks` apliquem a mesma correção de
/// fairness (ver o comentário em `task_writer_for`). Antes desta função
/// existir, `new_writer_pool()` criava seu PRÓPRIO writer único de força
/// fixa — hoje sem chamador em produção (`WriteRequest::Task` só é exercido
/// pelos testes da própria `writer_pool`), mas era uma bomba-relógio: um
/// refactor futuro que roteasse o claim loop por ali reintroduziria o bug de
/// 99,7%-para-um-agente-só, sem nenhum teste pra pegar.
#[cfg(feature = "dds")]
fn build_tasks_writer_pool(
    publisher: &Publisher,
    tasks_topic: &Topic<Task>,
    ownership_strength: i32,
) -> Result<Vec<DataWriter<Task>>, api::DataSpaceError> {
    // Só o papel AGENTE recebe mais de um writer (ver `task_writer_for` para
    // a motivação — corrige um desbalanceamento de carga real e reproduzido
    // entre agentes, medido em 94,8%/99,7% das tasks indo sempre para o
    // mesmo agente, documentado na dissertação §OP1/OP2 e confirmado
    // empiricamente nesta sessão com dois agentes mock: toda
    // `Ownership::Exclusive` empatada em strength cai num desempate
    // determinístico por GUID do writer — o MESMO agente vence toda disputa
    // pelo tempo de vida da conexão, não é acaso por task). Cliente/
    // orquestrador continuam com um único writer (comportamento inalterado —
    // não competem entre si pela mesma task).
    let pool_size = if ownership_strength == DataSpace::STRENGTH_AGENT {
        DataSpace::AGENT_TASKS_WRITER_POOL
    } else {
        1
    };
    // Seed por PROCESSO (não por task): precisa ser diferente entre agentes
    // para que a ordenação de força varie de agente para agente no mesmo
    // slot — um DefaultHasher (chave fixa) daria a MESMA seed pra todo
    // mundo, reproduzindo o bug original. Usa `RandomState` (chaves
    // aleatórias por processo, semeadas pelo SO — o mesmo mecanismo por trás
    // do `HashMap` default) em vez de misturar PID + horário manualmente: a
    // primeira tentativa (XOR de nanos com PID) não tinha entropia
    // suficiente nos bits baixos usados pelo `% K` quando os agentes eram
    // iniciados quase ao mesmo tempo (medido: 3 agentes concorrentes ainda
    // ficavam em ~22/28/50%, não ~33/33/33).
    let proc_seed: u64 = {
        use std::collections::hash_map::RandomState;
        use std::hash::{BuildHasher, Hasher};
        let mut h = RandomState::new().build_hasher();
        h.write_u32(std::process::id());
        h.finish()
    };
    let mut writers = Vec::with_capacity(pool_size);
    for slot in 0..pool_size {
        let strength = if pool_size > 1 {
            // Mistura (seed, slot) com SipHash (boa difusão de bits, ao
            // contrário de um XOR+multiply cru) antes do `% K` — o que
            // importa é variar por slot E por agente, não o valor absoluto.
            use std::collections::hash_map::DefaultHasher;
            use std::hash::{Hash, Hasher};
            let mut h = DefaultHasher::new();
            proc_seed.hash(&mut h);
            slot.hash(&mut h);
            let mixed = h.finish();
            ownership_strength + (mixed % 64) as i32
        } else {
            ownership_strength
        };
        let q_slot = qos::profiles::tasks(Some(strength)).map_err(err)?;
        let w = DataWriter::with_qos(publisher, tasks_topic, Some(&q_slot)).map_err(err)?;
        writers.push(w);
    }
    Ok(writers)
}

/// Escolhe, para um `task_id`, qual índice do pool de writers de `Tasks`
/// usar — compartilhado por `DataSpace::task_writer_for` e por
/// `writer_pool::make_write_fn` (o caminho `WriteRequest::Task`), para que os
/// DOIS pontos de escrita roteiem a MESMA task para o MESMO slot. Ver o
/// comentário em `task_writer_for` para por que o hash usa chave FIXA
/// (precisa ser igual em todos os processos).
#[cfg(feature = "dds")]
pub(crate) fn select_task_writer_slot(task_id: &str, pool_len: usize) -> usize {
    use std::collections::hash_map::DefaultHasher;
    use std::hash::{Hash, Hasher};
    if pool_len <= 1 {
        return 0;
    }
    let mut hasher = DefaultHasher::new();
    task_id.hash(&mut hasher);
    (hasher.finish() as usize) % pool_len
}

#[cfg(feature = "dds")]
fn create_participant(
    domain_id: u32,
    #[cfg(feature = "security")] security: Option<SecurityConfig>,
) -> Result<DomainParticipant, api::DataSpaceError> {
    #[cfg(feature = "security")]
    if let Some(sec) = security {
        let qos = cyclonedds::QosBuilder::new()
            .security(sec)
            // CycloneDDS exige as propriedades de plugin dynamic loader para
            // auth/crypto/access. A biblioteca rust configura os nomes dos
            // plugins built-in, mas não os entrypoints das bibliotecas nativas.
            .property("dds.sec.auth.library.path", "dds_security_auth")
            .property("dds.sec.auth.library.init", "init_authentication")
            .property("dds.sec.auth.library.finalize", "finalize_authentication")
            .property("dds.sec.crypto.library.path", "dds_security_crypto")
            .property("dds.sec.crypto.library.init", "init_crypto")
            .property("dds.sec.crypto.library.finalize", "finalize_crypto")
            .property("dds.sec.access.library.path", "dds_security_ac")
            .property("dds.sec.access.library.init", "init_access_control")
            .property("dds.sec.access.library.finalize", "finalize_access_control")
            .build()
            .map_err(err)?;
        return DomainParticipant::with_qos(domain_id, Some(&qos)).map_err(err);
    }
    DomainParticipant::new(domain_id).map_err(err)
}

#[cfg(feature = "dds")]
impl DataSpace {
    /// Strength por papel (Fase 2.2 já validada no Python): cliente<agente<orq.
    pub const STRENGTH_CLIENT: i32 = 10;
    pub const STRENGTH_AGENT: i32 = 100;
    pub const STRENGTH_ORCHESTRATOR: i32 = 200;

    /// Nº de writers de `Tasks` no pool de um DataSpace com papel de AGENTE
    /// (ver `task_writer_for`). Irrelevante para os demais papéis (pool de 1).
    ///
    /// Quanto maior, menor a variância de quantos slots cada agente "vence"
    /// por sorte (lei dos grandes números) — medido empiricamente: com 16,
    /// 3 agentes concorrentes ainda mostravam desbalanceamento visível
    /// (~22%/28%/50% em vez de ~33% cada).
    const AGENT_TASKS_WRITER_POOL: usize = 64;

    /// Sobe o DataSpace no domínio: participant + todos os tópicos canônicos + writers/readers.
    pub fn new(domain_id: u32, ownership_strength: i32) -> Result<Self, api::DataSpaceError> {
        Self::new_with_profile(domain_id, ownership_strength, None)
    }

    /// Sobe o DataSpace com QoS configurável por perfil (para campanha experimental).
    /// `profile_name`: Some("QoS_Balanced") para perfil específico, None para default.
    pub fn new_with_profile(
        domain_id: u32,
        ownership_strength: i32,
        profile_name: Option<&str>,
    ) -> Result<Self, api::DataSpaceError> {
        #[cfg(feature = "security")]
        return Self::new_with_profile_and_security(
            domain_id,
            ownership_strength,
            profile_name,
            None,
        );
        #[cfg(not(feature = "security"))]
        {
            let participant = create_participant(domain_id)?;
            Self::build_data_space(domain_id, participant, ownership_strength, profile_name)
        }
    }

    /// Sobe o DataSpace com perfil QoS opcional e configuração DDS Security.
    #[cfg(feature = "security")]
    pub fn new_with_profile_and_security(
        domain_id: u32,
        ownership_strength: i32,
        profile_name: Option<&str>,
        security: Option<SecurityConfig>,
    ) -> Result<Self, api::DataSpaceError> {
        let participant = create_participant(domain_id, security)?;
        Self::build_data_space(domain_id, participant, ownership_strength, profile_name)
    }

    fn build_data_space(
        domain_id: u32,
        participant: DomainParticipant,
        ownership_strength: i32,
        profile_name: Option<&str>,
    ) -> Result<Self, api::DataSpaceError> {
        let publisher = Publisher::new(&participant).map_err(err)?;
        let subscriber = Subscriber::new(&participant).map_err(err)?;

        // ── QoS profiles ────────────────────────────────────────────────
        let q_tasks = if let Some(profile) = profile_name {
            qos::profiles::tasks_with_profile(profile, Some(ownership_strength)).map_err(err)?
        } else {
            qos::profiles::tasks(Some(ownership_strength)).map_err(err)?
        };
        let q_agents = qos::profiles::agent_registry().map_err(err)?;
        let q_outputs = qos::profiles::task_output(Some(ownership_strength)).map_err(err)?;
        let q_system_metrics = qos::profiles::system_metrics().map_err(err)?;
        let q_server_status = qos::profiles::server_status().map_err(err)?;
        let q_llm = qos::profiles::llm().map_err(err)?;
        let q_llm_result = qos::profiles::llm_result().map_err(err)?;
        let q_ctx_snap = qos::profiles::context_snapshot().map_err(err)?;
        let q_ctx_upd = qos::profiles::context_update().map_err(err)?;
        let q_tool = qos::profiles::tool_call().map_err(err)?;
        let q_trace = qos::profiles::execution_trace().map_err(err)?;
        let q_sec_snap = qos::profiles::security_snapshot().map_err(err)?;
        let q_sec_upd = qos::profiles::security_update().map_err(err)?;
        let q_qos_route = qos::profiles::qos_routing().map_err(err)?;
        let q_qos_metric = qos::profiles::qos_metric().map_err(err)?;
        let q_qos_viol = qos::profiles::qos_violation().map_err(err)?;
        let q_disc = qos::profiles::qos_discovery().map_err(err)?;

        // ── Topics ───────────────────────────────────────────────────────
        let tasks_topic =
            Topic::<Task>::with_qos(&participant, topics::TASKS, Some(&q_tasks)).map_err(err)?;
        let agents_topic =
            Topic::<AgentState>::with_qos(&participant, topics::AGENT_REGISTRY, Some(&q_agents))
                .map_err(err)?;
        let outputs_topic =
            Topic::<TaskOutput>::with_qos(&participant, topics::TASK_OUTPUT, Some(&q_outputs))
                .map_err(err)?;
        let system_metrics_topic = Topic::<SystemMetric>::with_qos(
            &participant,
            topics::SYSTEM_METRICS,
            Some(&q_system_metrics),
        )
        .map_err(err)?;
        let server_status_topic = Topic::<ServerStatus>::with_qos(
            &participant,
            topics::SERVER_STATUS,
            Some(&q_server_status),
        )
        .map_err(err)?;

        let llm_request_topic =
            Topic::<LLMInferenceRequest>::with_qos(&participant, topics::LLM_REQUEST, Some(&q_llm))
                .map_err(err)?;
        let llm_result_topic = Topic::<LLMInferenceResult>::with_qos(
            &participant,
            topics::LLM_RESULT,
            Some(&q_llm_result),
        )
        .map_err(err)?;
        let llm_error_topic =
            Topic::<LLMInferenceError>::with_qos(&participant, topics::LLM_ERROR, Some(&q_llm))
                .map_err(err)?;

        let context_snapshot_topic = Topic::<ContextSnapshot>::with_qos(
            &participant,
            topics::CONTEXT_SNAPSHOT,
            Some(&q_ctx_snap),
        )
        .map_err(err)?;
        let context_update_topic = Topic::<ContextUpdate>::with_qos(
            &participant,
            topics::CONTEXT_UPDATE,
            Some(&q_ctx_upd),
        )
        .map_err(err)?;

        let tool_call_topic = Topic::<ToolCallRequest>::with_qos(
            &participant,
            topics::TOOL_CALL_REQUEST,
            Some(&q_tool),
        )
        .map_err(err)?;
        let execution_trace_topic = Topic::<ExecutionTraceEvent>::with_qos(
            &participant,
            topics::EXECUTION_TRACE,
            Some(&q_trace),
        )
        .map_err(err)?;

        let security_snapshot_topic = Topic::<SecurityPolicySnapshot>::with_qos(
            &participant,
            topics::SECURITY_POLICY_SNAPSHOT,
            Some(&q_sec_snap),
        )
        .map_err(err)?;
        let security_update_topic = Topic::<SecurityPolicyUpdate>::with_qos(
            &participant,
            topics::SECURITY_POLICY_UPDATE,
            Some(&q_sec_upd),
        )
        .map_err(err)?;

        let qos_routing_topic = Topic::<QoSRoutingProfile>::with_qos(
            &participant,
            topics::QOS_ROUTING_PROFILE,
            Some(&q_qos_route),
        )
        .map_err(err)?;
        let qos_metric_topic =
            Topic::<QoSMetric>::with_qos(&participant, topics::QOS_METRIC, Some(&q_qos_metric))
                .map_err(err)?;
        let qos_violation_topic =
            Topic::<QoSViolation>::with_qos(&participant, topics::QOS_VIOLATION, Some(&q_qos_viol))
                .map_err(err)?;
        let discovery_event_topic =
            Topic::<DiscoveryEvent>::with_qos(&participant, topics::QOS_DISCOVERY, Some(&q_disc))
                .map_err(err)?;

        // ── Writers ──────────────────────────────────────────────────────
        let tasks_writers = build_tasks_writer_pool(&publisher, &tasks_topic, ownership_strength)?;
        let agents_writer =
            DataWriter::with_qos(&publisher, &agents_topic, Some(&q_agents)).map_err(err)?;
        let outputs_writer =
            DataWriter::with_qos(&publisher, &outputs_topic, Some(&q_outputs)).map_err(err)?;
        let system_metrics_writer =
            DataWriter::with_qos(&publisher, &system_metrics_topic, Some(&q_system_metrics))
                .map_err(err)?;
        let server_status_writer =
            DataWriter::with_qos(&publisher, &server_status_topic, Some(&q_server_status))
                .map_err(err)?;

        let llm_request_writer =
            DataWriter::with_qos(&publisher, &llm_request_topic, Some(&q_llm)).map_err(err)?;
        let llm_result_writer =
            DataWriter::with_qos(&publisher, &llm_result_topic, Some(&q_llm_result))
                .map_err(err)?;
        let llm_error_writer =
            DataWriter::with_qos(&publisher, &llm_error_topic, Some(&q_llm)).map_err(err)?;

        let context_snapshot_writer =
            DataWriter::with_qos(&publisher, &context_snapshot_topic, Some(&q_ctx_snap))
                .map_err(err)?;
        let context_update_writer =
            DataWriter::with_qos(&publisher, &context_update_topic, Some(&q_ctx_upd))
                .map_err(err)?;

        let tool_call_writer =
            DataWriter::with_qos(&publisher, &tool_call_topic, Some(&q_tool)).map_err(err)?;
        let execution_trace_writer =
            DataWriter::with_qos(&publisher, &execution_trace_topic, Some(&q_trace))
                .map_err(err)?;

        let security_snapshot_writer =
            DataWriter::with_qos(&publisher, &security_snapshot_topic, Some(&q_sec_snap))
                .map_err(err)?;
        let security_update_writer =
            DataWriter::with_qos(&publisher, &security_update_topic, Some(&q_sec_upd))
                .map_err(err)?;

        let qos_routing_writer =
            DataWriter::with_qos(&publisher, &qos_routing_topic, Some(&q_qos_route))
                .map_err(err)?;
        let qos_metric_writer =
            DataWriter::with_qos(&publisher, &qos_metric_topic, Some(&q_qos_metric))
                .map_err(err)?;
        let qos_violation_writer =
            DataWriter::with_qos(&publisher, &qos_violation_topic, Some(&q_qos_viol))
                .map_err(err)?;
        let discovery_event_writer =
            DataWriter::with_qos(&publisher, &discovery_event_topic, Some(&q_disc)).map_err(err)?;

        // ── Readers ──────────────────────────────────────────────────────
        // Só `tasks_reader` é mantido como campo (usado por
        // `read_task_mesh`/confirmação de ownership). Os demais tópicos são
        // lidos exclusivamente via `stream_*`, que cria um reader 'static
        // dedicado por chamada (ver doc de `stream_tasks`) — manter aqui
        // seria um reader órfão, gastando entidade DDS + WaitSet à toa.
        let tasks_reader =
            DataReader::with_qos(&subscriber, &tasks_topic, Some(&q_tasks)).map_err(err)?;
        let tool_call_reader =
            DataReader::with_qos(&subscriber, &tool_call_topic, Some(&q_tool)).map_err(err)?;

        let shared_waitset = dispatch::SharedWaitSet::new(&participant).map_err(err)?;

        tracing::info!(
            domain_id,
            ownership_strength,
            "DataSpace iniciado com 18 tópicos"
        );
        Ok(Self {
            tasks_writers,
            agents_writer,
            outputs_writer,
            tasks_reader,
            tasks_topic: Arc::new(tasks_topic),
            agents_topic: Arc::new(agents_topic),
            outputs_topic: Arc::new(outputs_topic),
            system_metrics_writer,
            server_status_writer,
            system_metrics_topic: Arc::new(system_metrics_topic),
            server_status_topic: Arc::new(server_status_topic),

            llm_request_writer,
            llm_result_writer,
            llm_error_writer,
            llm_request_topic: Arc::new(llm_request_topic),
            llm_result_topic: Arc::new(llm_result_topic),
            llm_error_topic: Arc::new(llm_error_topic),

            context_snapshot_writer,
            context_update_writer,
            context_snapshot_topic: Arc::new(context_snapshot_topic),
            context_update_topic: Arc::new(context_update_topic),

            tool_call_writer,
            tool_call_topic: Arc::new(tool_call_topic),
            tool_call_reader,
            execution_trace_writer,
            execution_trace_topic: Arc::new(execution_trace_topic),

            security_snapshot_writer,
            security_update_writer,
            security_snapshot_topic: Arc::new(security_snapshot_topic),
            security_update_topic: Arc::new(security_update_topic),

            qos_routing_writer,
            qos_metric_writer,
            qos_violation_writer,
            discovery_event_writer,
            qos_routing_topic: Arc::new(qos_routing_topic),
            qos_metric_topic: Arc::new(qos_metric_topic),
            qos_violation_topic: Arc::new(qos_violation_topic),
            discovery_event_topic: Arc::new(discovery_event_topic),

            publisher: Arc::new(publisher),
            subscriber: Arc::new(subscriber),
            participant,
            ownership_strength,
            caches: Arc::new(TopicCaches::new()),
            shared_waitset,
        })
    }

    /// Escolhe, para um `task_id`, QUAL writer do pool usar (ver o comentário
    /// em `new()` sobre por que existe mais de um para o papel AGENTE).
    ///
    /// A escolha precisa ser a MESMA em todos os processos (todo agente tem
    /// que rotear o mesmo `task_id` para o slot de mesmo índice — só assim a
    /// arbitragem de `Ownership::Exclusive` fica bem definida: cada agente
    /// usa SEU writer daquele índice, cujas forças foram sorteadas
    /// independentemente por processo, então o vencedor varia de task para
    /// task em vez de ser sempre o mesmo agente. Por isso o hash usa
    /// `DefaultHasher::new()` (chaves fixas, reprodutível entre processos) e
    /// NÃO `RandomState`/`HashMap` (aleatorizado por processo, daria índices
    /// diferentes em cada agente e quebraria a garantia de exclusividade —
    /// dois agentes escrevendo em writers de força igual para o MESMO
    /// task_id sem nenhum deles saber do outro).
    fn task_writer_for(&self, task_id: &str) -> &DataWriter<Task> {
        &self.tasks_writers[select_task_writer_slot(task_id, self.tasks_writers.len())]
    }

    /// Lê o estado ARBITRADO do mesh para uma task (RHC do reader, não o cache).
    ///
    /// Usado na confirmação de ownership (T-203): o RHC mantém, por instância, a
    /// versão vencedora da arbitragem de Exclusive Ownership (maior strength;
    /// empate → menor GUID — determinístico e igual nos dois lados). O cache da
    /// aplicação NÃO serve para isso: por chegada, o próprio echo do 2º a clamar
    /// sempre venceria (execução dupla).
    pub fn read_task_mesh(&self, task_id: &str) -> Result<Option<Task>, api::DataSpaceError> {
        let key = Task {
            task_id: task_id.to_owned(),
            ..Task::default()
        };
        let handle = self.tasks_reader.lookup_instance(&key);
        if handle == 0 {
            return Ok(None);
        }

        let samples = self.tasks_reader.read_instance(handle).map_err(err)?;
        let samples = samples.to_vec().map_err(err)?;
        Ok(samples.into_iter().rev().map(|sample| sample.data).next())
    }

    pub fn read_tool_call_mesh(
        &self,
        call_id: &str,
    ) -> Result<Option<ToolCallRequest>, api::DataSpaceError> {
        let key = ToolCallRequest {
            call_id: call_id.to_owned(),
            ..ToolCallRequest::default()
        };
        let handle = self.tool_call_reader.lookup_instance(&key);
        if handle == 0 {
            return Ok(None);
        }
        let samples = self.tool_call_reader.read_instance(handle).map_err(err)?;
        let samples = samples.to_vec().map_err(err)?;
        Ok(samples.into_iter().rev().map(|sample| sample.data).next())
    }

    /// Aplica os knobs online do decisor de QoS no writer de `Tasks` (REQ-405).
    /// TransportPriority/LatencyBudget/OwnershipStrength são mutáveis em runtime.
    ///
    /// Só chamado hoje pelo papel ORQUESTRADOR (pool de 1 writer — ver
    /// `new()`); aplica em todos os writers do pool por generalidade, sem
    /// mudar o comportamento existente para pool de tamanho 1.
    pub fn apply_tasks_knobs(
        &self,
        knobs: &dds_contract::qos::OnlineKnobs,
    ) -> Result<(), api::DataSpaceError> {
        let qos =
            qos::profiles::tasks_with_knobs(Some(self.ownership_strength), knobs).map_err(err)?;
        for w in &self.tasks_writers {
            w.set_qos(&qos).map_err(err)?;
        }
        Ok(())
    }

    pub fn ownership_strength(&self) -> i32 {
        self.ownership_strength
    }

    /// Encerra o DataSpace (drop ordenado: filhos → tópicos → pub/sub → participant).
    pub async fn shutdown(self) -> Result<(), api::DataSpaceError> {
        tracing::info!("DataSpace encerrando");
        drop(self);
        Ok(())
    }

    // ── helpers síncronos mínimos (smoke T-302; a API async completa vem em T-303+) ──

    pub fn write_task_sync(&self, task: &Task) -> Result<(), api::DataSpaceError> {
        self.task_writer_for(&task.task_id).write(task).map_err(err)
    }

    pub fn take_tasks_sync(&self) -> Result<Vec<Task>, api::DataSpaceError> {
        self.tasks_reader.take().map_err(err)
    }
}

#[cfg(feature = "dds")]
fn err(e: cyclonedds::DdsError) -> api::DataSpaceError {
    api::DataSpaceError::Dds(e.to_string())
}

// ── Streams por evento (T-304, REQ-302/303) ────────────────────────────────

#[cfg(feature = "dds")]
use cache::TopicCaches;
#[cfg(feature = "dds")]
use futures_core::Stream;

#[cfg(feature = "dds")]
impl DataSpace {
    /// Handle compartilhado dos caches (alimentados pelas streams T-304 e
    /// pelos writers T-305).
    pub fn caches(&self) -> Arc<TopicCaches> {
        Arc::clone(&self.caches)
    }

    /// Handle do WaitSet compartilhado (Fase 5/T-617) — para
    /// observabilidade/testes de aceite (ver `tests/shared_waitset.rs`).
    pub fn shared_waitset(&self) -> Arc<dispatch::SharedWaitSet> {
        Arc::clone(&self.shared_waitset)
    }

    /// Stream de `Task` acordada por amostra (WaitSet compartilhado — Fase 5/T-617,
    /// ver `dispatch.rs` — sem polling). Cada chamada cria um reader dedicado
    /// ('static, sem corrida de take entre assinantes), anexado ao WaitSet
    /// único do `DataSpace`. O reader é criado e anexado antes de devolver o
    /// stream, para que um pump persistente já exista antes do primeiro write
    /// em um tópico Volatile. Cada amostra alimenta o cache (upsert monotônico).
    pub fn stream_tasks(&self) -> impl Stream<Item = cache::ArcTask> + 'static {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.tasks_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        let setup = match DataReader::with_qos(&subscriber, &topic, None) {
            Ok(reader) => match waitset.register(&reader) {
                Ok(registration) => Some((reader, registration)),
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(Tasks) falhou; stream encerrado");
                    None
                }
            },
            Err(e) => {
                tracing::error!(error = %e, "DataReader::with_qos(Tasks) falhou; stream encerrado");
                None
            }
        };
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(tasks) if !tasks.is_empty() => {
                        for t in tasks {
                            // RUST-CACHE-006: só entrega ao consumidor o que
                            // está de fato no cache (legível via read_task).
                            if let cache::TaskUpsert::Accepted(t) = caches.upsert_task(t) {
                                yield t;
                            }
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(Tasks) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `AgentState` acordada por amostra (heartbeat dos agentes).
    pub fn stream_agent_states(&self) -> impl Stream<Item = cache::ArcAgentState> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.agents_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(AgentRegistry) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(AgentRegistry) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(states) if !states.is_empty() => {
                        for s in states {
                            yield caches.upsert_agent(s);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(AgentRegistry) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `TaskOutput` acordada por amostra (chunks de inferência).
    /// O reader é criado e anexado antes de devolver o stream, para não perder
    /// amostras Volatile quando o pump do cliente ainda não foi polled.
    pub fn stream_task_outputs(&self) -> impl Stream<Item = cache::ArcTaskOutput> + 'static {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.outputs_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        let setup = match DataReader::with_qos(&subscriber, &topic, None) {
            Ok(reader) => match waitset.register(&reader) {
                Ok(registration) => Some((reader, registration)),
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(TaskOutput) falhou; stream encerrado");
                    None
                }
            },
            Err(e) => {
                tracing::error!(error = %e, "DataReader::with_qos(TaskOutput) falhou; stream encerrado");
                None
            }
        };
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(outs) if !outs.is_empty() => {
                        for o in outs {
                            yield caches.push_output(o);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(TaskOutput) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `LLMInferenceRequest` acordada por amostra.
    pub fn stream_llm_requests(&self) -> impl Stream<Item = cache::ArcLLMRequest> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.llm_request_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(LLMRequest) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(LLMRequest) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(reqs) if !reqs.is_empty() => {
                        for r in reqs {
                            yield caches.upsert_llm_request(r);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(LLMRequest) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `LLMInferenceResult` acordada por amostra.
    pub fn stream_llm_results(&self) -> impl Stream<Item = cache::ArcLLMResult> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.llm_result_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(LLMResult) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(LLMResult) falhou; stream encerrado");
                    return;
                }
            };
            // Padrão enable-antes-de-drenar (sem wakeup perdido): uma
            // notificação level-triggered cobre TUDO o que está no RHC —
            // drena até esvaziar. Notificações que chegarem durante o dreno
            // ficam capturadas no `Notified` habilitado e disparam um novo
            // ciclo de dreno. Antes (1 take por notificação), bursts
            // perdiam amostras no meio do stream (medido: 27–30/128).
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(results) if !results.is_empty() => {
                            for r in results {
                                yield caches.push_llm_result(r);
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(LLMResult) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `LLMInferenceError` acordada por amostra.
    pub fn stream_llm_errors(&self) -> impl Stream<Item = cache::ArcLLMError> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.llm_error_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(LLMError) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(LLMError) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(errors) if !errors.is_empty() => {
                        for e in errors {
                            yield caches.upsert_llm_error(e);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(LLMError) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `ContextSnapshot` acordada por amostra.
    pub fn stream_context_snapshots(&self) -> impl Stream<Item = cache::ArcContextSnapshot> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.context_snapshot_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(ContextSnapshot) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(ContextSnapshot) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(snaps) if !snaps.is_empty() => {
                        for s in snaps {
                            yield caches.upsert_context_snapshot(s);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(ContextSnapshot) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `ContextUpdate` acordada por amostra.
    pub fn stream_context_updates(&self) -> impl Stream<Item = cache::ArcContextUpdate> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.context_update_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(ContextUpdate) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(ContextUpdate) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(updates) if !updates.is_empty() => {
                        for u in updates {
                            yield caches.push_context_update(u);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(ContextUpdate) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `ToolCallRequest` acordada por amostra.
    pub fn stream_tool_calls(&self) -> impl Stream<Item = cache::ArcToolCallRequest> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.tool_call_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(ToolCall) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(ToolCall) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(calls) if !calls.is_empty() => {
                        for c in calls {
                            yield caches.upsert_tool_call(c);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(ToolCall) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `ExecutionTraceEvent` acordada por amostra.
    pub fn stream_execution_traces(&self) -> impl Stream<Item = cache::ArcExecutionTraceEvent> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.execution_trace_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(ExecutionTrace) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(ExecutionTrace) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(events) if !events.is_empty() => {
                        for e in events {
                            yield caches.push_execution_trace(e);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(ExecutionTrace) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `SecurityPolicySnapshot` acordada por amostra.
    pub fn stream_security_snapshots(
        &self,
    ) -> impl Stream<Item = cache::ArcSecurityPolicySnapshot> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.security_snapshot_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(SecuritySnapshot) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(SecuritySnapshot) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(snaps) if !snaps.is_empty() => {
                        for s in snaps {
                            caches.upsert_security_snapshot(s.clone());
                            yield Arc::new(s);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(SecuritySnapshot) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `SecurityPolicyUpdate` acordada por amostra.
    pub fn stream_security_updates(&self) -> impl Stream<Item = cache::ArcSecurityPolicyUpdate> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.security_update_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(SecurityUpdate) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(SecurityUpdate) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(updates) if !updates.is_empty() => {
                        for u in updates {
                            yield caches.push_security_update(u);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(SecurityUpdate) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `QoSRoutingProfile` acordada por amostra.
    pub fn stream_qos_routing(&self) -> impl Stream<Item = cache::ArcQoSRoutingProfile> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.qos_routing_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(QoSRouting) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(QoSRouting) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(profiles) if !profiles.is_empty() => {
                        for p in profiles {
                            yield caches.upsert_qos_routing(p);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(QoSRouting) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `QoSMetric` acordada por amostra.
    pub fn stream_qos_metrics(&self) -> impl Stream<Item = cache::ArcQoSMetric> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.qos_metric_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(QoSMetric) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(QoSMetric) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(metrics) if !metrics.is_empty() => {
                        for m in metrics {
                            yield caches.upsert_qos_metric(m);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(QoSMetric) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `QoSViolation` acordada por amostra.
    pub fn stream_qos_violations(&self) -> impl Stream<Item = cache::ArcQoSViolation> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.qos_violation_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(QoSViolation) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(QoSViolation) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(violations) if !violations.is_empty() => {
                        for v in violations {
                            yield caches.upsert_qos_violation(v);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(QoSViolation) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Stream de `DiscoveryEvent` acordada por amostra.
    pub fn stream_discovery_events(&self) -> impl Stream<Item = cache::ArcDiscoveryEvent> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.discovery_event_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let reader = match DataReader::with_qos(&subscriber, &topic, None) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "DataReader::with_qos(DiscoveryEvent) falhou; stream encerrado");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(r) => r,
                Err(e) => {
                    tracing::error!(error = %e, "waitset.register(DiscoveryEvent) falhou; stream encerrado");
                    return;
                }
            };
            loop {
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(events) if !events.is_empty() => {
                        for e in events {
                            yield caches.upsert_discovery_event(e);
                        }
                    }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(error = %e, "take_async(DiscoveryEvent) falhou; retry");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                n.await;
            }
        }
    }

    /// Streams `SystemMetrics` using the shared event-driven WaitSet (REQ-708).
    pub fn stream_system_metrics(&self) -> impl Stream<Item = cache::ArcSystemMetric> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.system_metrics_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let profile = match qos::profiles::system_metrics() {
                Ok(profile) => profile,
                Err(error) => {
                    tracing::error!(%error, "SystemMetrics reader QoS failed");
                    return;
                }
            };
            let reader = match DataReader::with_qos(&subscriber, &topic, Some(&profile)) {
                Ok(reader) => reader,
                Err(error) => {
                    tracing::error!(%error, "DataReader::with_qos(SystemMetrics) failed");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(registration) => registration,
                Err(error) => {
                    tracing::error!(%error, "waitset.register(SystemMetrics) failed");
                    return;
                }
            };
            loop {
                let notified = registration.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(metrics) if !metrics.is_empty() => {
                            for metric in metrics {
                                yield caches.upsert_system_metric(metric);
                            }
                        }
                        Ok(_) => break,
                        Err(error) => {
                            tracing::warn!(%error, "take_async(SystemMetrics) failed; retrying");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                notified.await;
            }
        }
    }

    /// Streams `ServerStatus` using the shared event-driven WaitSet (REQ-708).
    pub fn stream_server_status(&self) -> impl Stream<Item = cache::ArcServerStatus> {
        let caches = self.caches();
        let subscriber = Arc::clone(&self.subscriber);
        let topic = Arc::clone(&self.server_status_topic);
        let waitset = Arc::clone(&self.shared_waitset);
        async_stream::stream! {
            let profile = match qos::profiles::server_status() {
                Ok(profile) => profile,
                Err(error) => {
                    tracing::error!(%error, "ServerStatus reader QoS failed");
                    return;
                }
            };
            let reader = match DataReader::with_qos(&subscriber, &topic, Some(&profile)) {
                Ok(reader) => reader,
                Err(error) => {
                    tracing::error!(%error, "DataReader::with_qos(ServerStatus) failed");
                    return;
                }
            };
            let registration = match waitset.register(&reader) {
                Ok(registration) => registration,
                Err(error) => {
                    tracing::error!(%error, "waitset.register(ServerStatus) failed");
                    return;
                }
            };
            loop {
                let notified = registration.notified();
                tokio::pin!(notified);
                notified.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(statuses) if !statuses.is_empty() => {
                            for status in statuses {
                                yield caches.upsert_server_status(status);
                            }
                        }
                        Ok(_) => break,
                        Err(error) => {
                            tracing::warn!(%error, "take_async(ServerStatus) failed; retrying");
                            tokio::time::sleep(std::time::Duration::from_millis(100)).await;
                            break;
                        }
                    }
                }
                notified.await;
            }
        }
    }
}

// ── Pool de writers (T-305) ────────────────────────────────────────────────

#[cfg(feature = "dds")]
pub mod writer_pool;

// ── Monitor de QoS (T-306) ─────────────────────────────────────────────────

#[cfg(feature = "dds")]
pub mod monitor;

#[cfg(feature = "dds")]
impl DataSpace {
    /// Reader de `AgentRegistry` com QoS e listener custom (monitor/T-306).
    pub fn agents_reader_with(
        &self,
        qos: &cyclonedds::Qos,
        listener: &cyclonedds::Listener,
    ) -> DataReader<AgentState> {
        DataReader::with_qos_and_listener(
            &self.subscriber,
            &self.agents_topic,
            Some(qos),
            Some(listener),
        )
        .expect("reader AgentRegistry com listener — erro fatal na inicialização do monitor")
    }

    /// Reader de `TaskOutput` com QoS e listener custom (monitor/T-306).
    pub fn outputs_reader_with(
        &self,
        qos: &cyclonedds::Qos,
        listener: &cyclonedds::Listener,
    ) -> DataReader<TaskOutput> {
        DataReader::with_qos_and_listener(
            &self.subscriber,
            &self.outputs_topic,
            Some(qos),
            Some(listener),
        )
        .expect("reader TaskOutput com listener")
    }

    /// Writer de `AgentRegistry` com QoS custom (testes do monitor).
    pub fn agents_writer_with(&self, qos: &cyclonedds::Qos) -> DataWriter<AgentState> {
        DataWriter::with_qos(&self.publisher, &self.agents_topic, Some(qos))
            .expect("writer AgentRegistry")
    }

    /// Writer de `TaskOutput` com QoS custom (testes do monitor).
    pub fn outputs_writer_with(&self, qos: &cyclonedds::Qos) -> DataWriter<TaskOutput> {
        DataWriter::with_qos(&self.publisher, &self.outputs_topic, Some(qos))
            .expect("writer TaskOutput")
    }

    /// Writer de `Tasks` com QoS custom (ex.: papel cliente=10 para submissões
    /// da API — se fosse 200, os claims dos agentes perderiam a arbitragem).
    pub fn tasks_writer_with(&self, qos: &cyclonedds::Qos) -> DataWriter<Task> {
        DataWriter::with_qos(&self.publisher, &self.tasks_topic, Some(qos)).expect("writer Tasks")
    }
}

#[cfg(feature = "dds")]
impl DataSpace {
    /// Pool de escrita com writers dedicados (mesmos perfis/strength do DataSpace).
    pub fn new_writer_pool(&self, n_workers: usize, capacity: usize) -> writer_pool::WriterPool {
        let s = self.ownership_strength;
        let q_agents = qos::profiles::agent_registry().expect("qos agents");
        let q_outputs = qos::profiles::task_output(Some(s)).expect("qos outputs");

        // Mesmo pool com força variada por slot que `DataSpace::new()` usa —
        // ver `build_tasks_writer_pool`. Sem isso, `WriteRequest::Task`
        // (hoje só exercido pelos testes de `writer_pool`) reintroduziria o
        // desbalanceamento de carga entre agentes corrigido nesta sessão,
        // caso algum refactor futuro passe a rotear o claim loop por aqui.
        let tw = build_tasks_writer_pool(&self.publisher, &self.tasks_topic, s)
            .expect("writers Tasks do pool");
        let aw = DataWriter::with_qos(&self.publisher, &self.agents_topic, Some(&q_agents))
            .expect("writer AgentRegistry do pool");
        let ow = DataWriter::with_qos(&self.publisher, &self.outputs_topic, Some(&q_outputs))
            .expect("writer TaskOutput do pool");

        writer_pool::WriterPool::new(n_workers, capacity, writer_pool::make_write_fn(tw, aw, ow))
    }
}

// ── DataSpaceApi para o DataSpace real (T-307) ─────────────────────────────

#[cfg(feature = "dds")]
#[async_trait::async_trait]
impl api::DataSpaceApi for DataSpace {
    async fn write_task(&self, task: Task) -> Result<(), api::DataSpaceError> {
        // SEM write-through: o cache é alimentado APENAS pelas streams (visão do
        // mesh). Write-through tornaria o readback de claim inútil — o 2º a clamar
        // sempre se auto-confirmaria (execução dupla). read-after-write é
        // eventualmente consistente (~ms, entregue pela stream).
        //
        // Roteado por `task_writer_for`: todas as escritas do ciclo de vida
        // desta task (PENDING do cliente, ASSIGNED/RUNNING/DONE do agente
        // vencedor) precisam sair pelo MESMO writer (mesmo slot), senão a
        // arbitragem de Exclusive Ownership vê um writer novo/desconhecido
        // para a instância e rejeita — ver `task_writer_for` e o comentário
        // em `new()` sobre o pool de writers do papel AGENTE.
        self.task_writer_for(&task.task_id)
            .write(&task)
            .map_err(err)
    }

    async fn read_task(&self, task_id: &str) -> Result<Option<Arc<Task>>, api::DataSpaceError> {
        Ok(self.caches.read_task(task_id))
    }

    async fn all_tasks(&self) -> Result<Vec<Arc<Task>>, api::DataSpaceError> {
        Ok(self.caches.all_tasks())
    }

    fn subscribe_tasks(&self) -> std::pin::Pin<Box<dyn Stream<Item = Arc<Task>> + Send>> {
        Box::pin(self.stream_tasks())
    }

    async fn write_agent_state(&self, state: AgentState) -> Result<(), api::DataSpaceError> {
        // Sem write-through (mesma razão de write_task): cache alimentado pela stream.
        self.agents_writer.write(&state).map_err(err)
    }

    async fn read_agent_state(
        &self,
        agent_id: &str,
    ) -> Result<Option<AgentState>, api::DataSpaceError> {
        Ok(self.caches.read_agent(agent_id).map(|a| (*a).clone()))
    }

    async fn all_agents(&self) -> Result<Vec<AgentState>, api::DataSpaceError> {
        Ok(self
            .caches
            .all_agents()
            .iter()
            .map(|a| (**a).clone())
            .collect())
    }

    fn subscribe_agent_states(&self) -> std::pin::Pin<Box<dyn Stream<Item = AgentState> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_agent_states().map(|a| (*a).clone()))
    }

    async fn write_task_output(&self, output: TaskOutput) -> Result<(), api::DataSpaceError> {
        // Sem write-through (mesma razão de write_task): cache alimentado pela stream.
        self.outputs_writer.write(&output).map_err(err)
    }

    async fn read_task_outputs(
        &self,
        task_id: &str,
    ) -> Result<Vec<Arc<TaskOutput>>, api::DataSpaceError> {
        Ok(self.caches.outputs_of(task_id))
    }

    fn subscribe_task_outputs(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = Arc<TaskOutput>> + Send>> {
        Box::pin(self.stream_task_outputs())
    }

    async fn write_system_metric(&self, metric: SystemMetric) -> Result<(), api::DataSpaceError> {
        self.system_metrics_writer.write(&metric).map_err(err)
    }

    async fn read_system_metric(
        &self,
        metric_name: &str,
        component_id: &str,
    ) -> Result<Option<SystemMetric>, api::DataSpaceError> {
        Ok(self
            .caches
            .read_system_metric(metric_name, component_id)
            .map(|metric| (*metric).clone()))
    }

    fn subscribe_system_metrics(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = SystemMetric> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_system_metrics().map(|metric| (*metric).clone()))
    }

    async fn write_server_status(&self, status: ServerStatus) -> Result<(), api::DataSpaceError> {
        self.server_status_writer.write(&status).map_err(err)
    }

    async fn read_server_status(
        &self,
        server_id: &str,
    ) -> Result<Option<ServerStatus>, api::DataSpaceError> {
        Ok(self
            .caches
            .read_server_status(server_id)
            .map(|status| (*status).clone()))
    }

    fn subscribe_server_status(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = ServerStatus> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_server_status().map(|status| (*status).clone()))
    }

    async fn shutdown(&self) -> Result<(), api::DataSpaceError> {
        // Teardown real é via drop (RAII); aqui limpamos os caches (paridade com o mock).
        self.caches.tasks.clear();
        self.caches.agents.clear();
        self.caches.outputs.clear();
        self.caches.system_metrics.clear();
        self.caches.server_status.clear();
        self.caches.llm_requests.clear();
        self.caches.llm_results.clear();
        self.caches.llm_errors.clear();
        self.caches.context_snapshots.clear();
        self.caches.context_updates.clear();
        self.caches.tool_calls.clear();
        self.caches.execution_traces.clear();
        self.caches.security_snapshots.clear();
        self.caches.security_updates.clear();
        self.caches.qos_routing.clear();
        self.caches.qos_metrics.clear();
        self.caches.qos_violations.clear();
        self.caches.discovery_events.clear();
        Ok(())
    }

    // ── LLM methods ─────────────────────────────────────────────────────

    async fn write_llm_request(&self, req: LLMInferenceRequest) -> Result<(), api::DataSpaceError> {
        self.llm_request_writer.write(&req).map_err(err)
    }

    async fn write_llm_result(
        &self,
        result: LLMInferenceResult,
    ) -> Result<(), api::DataSpaceError> {
        self.llm_result_writer.write(&result).map_err(err)
    }

    async fn write_llm_error(&self, error: LLMInferenceError) -> Result<(), api::DataSpaceError> {
        self.llm_error_writer.write(&error).map_err(err)
    }

    fn subscribe_llm_requests(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = LLMInferenceRequest> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_llm_requests().map(|a| (*a).clone()))
    }

    fn subscribe_llm_results(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = LLMInferenceResult> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_llm_results().map(|a| (*a).clone()))
    }

    fn subscribe_llm_errors(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = LLMInferenceError> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_llm_errors().map(|a| (*a).clone()))
    }

    // ── Context methods ─────────────────────────────────────────────────

    async fn write_context_snapshot(
        &self,
        snap: ContextSnapshot,
    ) -> Result<(), api::DataSpaceError> {
        self.context_snapshot_writer.write(&snap).map_err(err)
    }

    async fn write_context_update(&self, update: ContextUpdate) -> Result<(), api::DataSpaceError> {
        self.context_update_writer.write(&update).map_err(err)
    }

    fn subscribe_context_snapshots(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = ContextSnapshot> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_context_snapshots().map(|a| (*a).clone()))
    }

    fn subscribe_context_updates(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = ContextUpdate> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_context_updates().map(|a| (*a).clone()))
    }

    // ── ToolCall methods ────────────────────────────────────────────────

    async fn write_tool_call(&self, call: ToolCallRequest) -> Result<(), api::DataSpaceError> {
        self.tool_call_writer.write(&call).map_err(err)
    }

    async fn read_tool_call(
        &self,
        call_id: &str,
    ) -> Result<Option<ToolCallRequest>, api::DataSpaceError> {
        Ok(self.read_tool_call_mesh(call_id)?)
    }

    fn subscribe_tool_calls(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = ToolCallRequest> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_tool_calls().map(|a| (*a).clone()))
    }

    // ── ExecutionTrace methods ──────────────────────────────────────────

    async fn write_execution_trace(
        &self,
        event: ExecutionTraceEvent,
    ) -> Result<(), api::DataSpaceError> {
        self.execution_trace_writer.write(&event).map_err(err)
    }

    fn subscribe_execution_traces(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = ExecutionTraceEvent> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_execution_traces().map(|a| (*a).clone()))
    }

    // ── Security methods ────────────────────────────────────────────────

    async fn write_security_snapshot(
        &self,
        snap: SecurityPolicySnapshot,
    ) -> Result<(), api::DataSpaceError> {
        self.security_snapshot_writer.write(&snap).map_err(err)
    }

    async fn write_security_update(
        &self,
        update: SecurityPolicyUpdate,
    ) -> Result<(), api::DataSpaceError> {
        self.security_update_writer.write(&update).map_err(err)
    }

    fn subscribe_security_snapshots(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = SecurityPolicySnapshot> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_security_snapshots().map(|a| (*a).clone()))
    }

    fn subscribe_security_updates(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = SecurityPolicyUpdate> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_security_updates().map(|a| (*a).clone()))
    }

    // ── QoS methods ─────────────────────────────────────────────────────

    async fn write_qos_routing(
        &self,
        profile: QoSRoutingProfile,
    ) -> Result<(), api::DataSpaceError> {
        self.qos_routing_writer.write(&profile).map_err(err)
    }

    async fn write_qos_metric(&self, metric: QoSMetric) -> Result<(), api::DataSpaceError> {
        self.qos_metric_writer.write(&metric).map_err(err)
    }

    async fn write_qos_violation(
        &self,
        violation: QoSViolation,
    ) -> Result<(), api::DataSpaceError> {
        self.qos_violation_writer.write(&violation).map_err(err)
    }

    async fn write_discovery_event(
        &self,
        event: DiscoveryEvent,
    ) -> Result<(), api::DataSpaceError> {
        self.discovery_event_writer.write(&event).map_err(err)
    }

    fn subscribe_qos_routing(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = QoSRoutingProfile> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_qos_routing().map(|a| (*a).clone()))
    }

    fn subscribe_qos_metrics(&self) -> std::pin::Pin<Box<dyn Stream<Item = QoSMetric> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_qos_metrics().map(|a| (*a).clone()))
    }

    fn subscribe_qos_violations(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = QoSViolation> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_qos_violations().map(|a| (*a).clone()))
    }

    fn subscribe_discovery_events(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = DiscoveryEvent> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_discovery_events().map(|a| (*a).clone()))
    }
}

#[cfg(not(feature = "dds"))]
pub struct DataSpace {
    pub ownership_strength: i32,
    pub domain_id: u32,
}

#[cfg(not(feature = "dds"))]
impl DataSpace {
    pub const STRENGTH_CLIENT: i32 = 10;
    pub const STRENGTH_AGENT: i32 = 100;
    pub const STRENGTH_ORCHESTRATOR: i32 = 200;

    pub fn new(domain_id: u32, ownership_strength: i32) -> Self {
        Self {
            ownership_strength,
            domain_id,
        }
    }

    pub fn ownership_strength(&self) -> i32 {
        self.ownership_strength
    }

    pub async fn shutdown(self) -> Result<(), crate::api::DataSpaceError> {
        Ok(())
    }
}
