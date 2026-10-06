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
pub mod shutdown;

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
    QoSRoutingProfile, QoSViolation, SecurityPolicySnapshot, SecurityPolicyUpdate,
    StudioNodePresence, SystemMetric, Task, TaskOutput, ToolCallRequest,
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
    /// Writer de `Tasks` com strength do CLIENTE (10) — path de publicação
    /// SEM assumir ownership (T-820-03): é por aqui que o reaper/reatribuição
    /// publica, para que um agente (strength 100–163) volte a vencer a
    /// arbitragem no claim seguinte. Ver `write_task_without_ownership`.
    tasks_writer_client: DataWriter<Task>,
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
    /// T-890-06: writer de strength CLIENTE — quem PÕE o pedido (orquestrador/
    /// sonda) publica sem ser dono; o gateway evolui a instância com força de
    /// agente (espelho do `tasks_writer_client`).
    tool_call_writer_client: DataWriter<ToolCallRequest>,
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

    // Tópico Studio.NodePresence (1 — T-890, 19º tópico)
    studio_node_presence_writer: DataWriter<StudioNodePresence>,
    studio_node_presence_topic: Arc<Topic<StudioNodePresence>>,

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

/// Hash FNV-1a 64-bit explícito (T-820-20).
///
/// Substitui o `DefaultHasher` do roteamento do pool de writers: o std NÃO
/// garante o algoritmo (nem as sementes) do `DefaultHasher` entre versões —
/// dois binários compilados com toolchains diferentes podiam rotear o MESMO
/// `task_id` para índices DIFERENTES, quebrando o invariante "mesmo task_id →
/// mesmo slot em todos os processos" em que a arbitragem de Exclusive
/// Ownership se apoia (ver `select_task_writer_slot`). FNV-1a é minúsculo,
/// sem dependências e especificado de forma fixa (offset `0xcbf29ce484222325`,
/// primo `0x100000001b3`).
#[must_use]
pub fn fnv1a64(bytes: &[u8]) -> u64 {
    const OFFSET_BASIS: u64 = 0xcbf2_9ce4_8422_2325;
    const PRIME: u64 = 0x0000_0100_0000_01b3;
    let mut hash = OFFSET_BASIS;
    for &byte in bytes {
        hash ^= u64::from(byte);
        hash = hash.wrapping_mul(PRIME);
    }
    hash
}

/// Escolhe, para um `task_id`, qual índice do pool de writers de `Tasks`
/// usar — compartilhado por `DataSpace::task_writer_for` e por
/// `writer_pool::make_write_fn` (o caminho `WriteRequest::Task`), para que os
/// DOIS pontos de escrita roteiem a MESMA task para o MESMO slot. Ver o
/// comentário em `task_writer_for` para por que o hash usa chave FIXA
/// (precisa ser igual em todos os processos). T-820-20: o hash é FNV-1a 64
/// explícito ([`fnv1a64`]) e não `DefaultHasher` — o algoritmo do std pode
/// mudar entre toolchains e o invariante cross-processo exige estabilidade.
#[cfg(feature = "dds")]
pub(crate) fn select_task_writer_slot(task_id: &str, pool_len: usize) -> usize {
    if pool_len <= 1 {
        return 0;
    }
    (fnv1a64(task_id.as_bytes()) as usize) % pool_len
}

/// Strength por papel (Fase 2.2 já validada no Python): cliente<agente<orq.
///
/// Fonte única (L1): aliases de `dds_contract::roles` — vale para o
/// `DataSpace` real (arbitragem EXCLUSIVE no DDS) e como referência
/// documental para o mock, que NÃO arbitra (ver `in_memory::InMemoryDataSpace`).
impl DataSpace {
    pub const STRENGTH_CLIENT: i32 = dds_contract::STRENGTH_CLIENT;
    pub const STRENGTH_AGENT: i32 = dds_contract::STRENGTH_AGENT;
    pub const STRENGTH_ORCHESTRATOR: i32 = dds_contract::STRENGTH_ORCHESTRATOR;
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
        let q_tool =
            qos::profiles::tool_call_with_strength(Some(DataSpace::STRENGTH_AGENT)).map_err(err)?;
        let q_tool_client =
            qos::profiles::tool_call_with_strength(Some(DataSpace::STRENGTH_CLIENT))
                .map_err(err)?;
        let q_trace = qos::profiles::execution_trace().map_err(err)?;
        let q_sec_snap = qos::profiles::security_snapshot().map_err(err)?;
        let q_sec_upd = qos::profiles::security_update().map_err(err)?;
        let q_qos_route = qos::profiles::qos_routing().map_err(err)?;
        let q_qos_metric = qos::profiles::qos_metric().map_err(err)?;
        let q_qos_viol = qos::profiles::qos_violation().map_err(err)?;
        let q_disc = qos::profiles::qos_discovery().map_err(err)?;
        let q_studio = qos::profiles::studio_node_presence().map_err(err)?;

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
        // T-890: 19º tópico — presença das instalações do Studio.
        let studio_node_presence_topic = Topic::<StudioNodePresence>::with_qos(
            &participant,
            topics::STUDIO_NODE_PRESENCE,
            Some(&q_studio),
        )
        .map_err(err)?;

        // ── Writers ──────────────────────────────────────────────────────
        let tasks_writers = build_tasks_writer_pool(&publisher, &tasks_topic, ownership_strength)?;
        // T-820-03: um ÚNICO writer de strength CLIENTE criado no boot (não
        // um por escrita) — o reaper/reatribuição publica por ele para não
        // se tornar dono da instância. Mesmo perfil usado pelo
        // `api_tasks_writer` do orquestrador (`qos::profiles::tasks` +
        // STRENGTH_CLIENT), para que mock/real e os dois caminhos de
        // publicação falem a mesma QoS.
        let tasks_writer_client = DataWriter::with_qos(
            &publisher,
            &tasks_topic,
            Some(&qos::profiles::tasks(Some(DataSpace::STRENGTH_CLIENT)).map_err(err)?),
        )
        .map_err(err)?;
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
        let tool_call_writer_client =
            DataWriter::with_qos(&publisher, &tool_call_topic, Some(&q_tool_client))
                .map_err(err)?;
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
        let studio_node_presence_writer =
            DataWriter::with_qos(&publisher, &studio_node_presence_topic, Some(&q_studio))
                .map_err(err)?;

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
            "DataSpace iniciado com 19 tópicos"
        );
        Ok(Self {
            tasks_writers,
            tasks_writer_client,
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
            tool_call_writer_client,
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

            studio_node_presence_writer,
            studio_node_presence_topic: Arc::new(studio_node_presence_topic),

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
    /// task em vez de ser sempre o mesmo agente. Por isso o hash é
    /// determinístico e estável entre toolchains — [`fnv1a64`], T-820-20 — e
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
    /// O `Drop` do `SharedWaitSet` marca a flag de shutdown e acorda os
    /// registros: as `stream_*` penduradas terminam graciosamente
    /// (`Registration::is_shutdown` — T-820-05/P1-7).
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

    /// Publica um heartbeat de `Studio.NodePresence` (T-890). Escrita
    /// síncrona — o heartbeat de 5 s não justifica o custo de um pool async;
    /// quem chama (studio-node) roda em task Tokio própria e o `write` do
    /// runtime CycloneDDS é não-bloqueante na prática (mesmo caminho dos
    /// demais writers do DataSpace).
    pub fn write_studio_node_presence(
        &self,
        presence: &StudioNodePresence,
    ) -> Result<(), api::DataSpaceError> {
        self.studio_node_presence_writer
            .write(presence)
            .map_err(err)
    }
}

#[cfg(feature = "dds")]
fn err(e: cyclonedds::DdsError) -> api::DataSpaceError {
    api::DataSpaceError::Dds(e.to_string())
}

// ── Streams por evento (T-304, REQ-302/303) ────────────────────────────────

#[cfg(feature = "dds")]
mod streams {
    //! Helpers compartilhados pelas `stream_*` (T-820-05): setup eager com o
    //! perfil QoS do tópico + backoff do retry de `take_async`.

    /// Backoff do retry de `take_async` nas streams (T-820-05/P3): começa em
    /// 100 ms e dobra até 5 s — antes era warn+100 ms fixos para sempre, que
    /// enche o log e queima CPU em falha persistente (ex.: reader órfão de
    /// um waitset derrubado).
    pub(super) const STREAM_TAKE_BACKOFF_INITIAL: std::time::Duration =
        std::time::Duration::from_millis(100);
    pub(super) const STREAM_TAKE_BACKOFF_MAX: std::time::Duration =
        std::time::Duration::from_secs(5);
}

#[cfg(feature = "dds")]
use streams::{STREAM_TAKE_BACKOFF_INITIAL, STREAM_TAKE_BACKOFF_MAX};

/// Setup SÍNCRONO de uma stream (T-820-05, P1-3): cria o reader com o perfil
/// QoS do TÓPICO — o mesmo usado pelos writers, não os defaults DDS
/// (BEST_EFFORT + KeepLast(1)), que perdiam amostras silenciosamente contra
/// writers Reliable/TransientLocal — e o registra no WaitSet compartilhado
/// ANTES de devolver o stream. O padrão era o de `stream_tasks`; agora TODAS
/// as `stream_*` passam por aqui: o setup preguiçoso (reader criado dentro do
/// gerador, no primeiro poll) perdia a janela entre criar a stream e o
/// primeiro poll nos tópicos Volatile (Context.Update, QoS.Discovery).
/// Falível por etapa: `None` = stream encerrada (o gerador nem inicia).
#[cfg(feature = "dds")]
fn setup_stream_reader<T: cyclonedds::DdsType>(
    label: &'static str,
    subscriber: &Subscriber,
    topic: &Topic<T>,
    waitset: &dispatch::SharedWaitSet,
    qos_profile: cyclonedds::DdsResult<cyclonedds::Qos>,
) -> Option<(DataReader<T>, dispatch::Registration)> {
    let qos = match qos_profile {
        Ok(q) => Some(q),
        Err(e) => {
            tracing::error!(
                topic = label,
                error = %e,
                "perfil QoS do reader falhou; stream encerrado"
            );
            return None;
        }
    };
    let reader = match DataReader::with_qos(subscriber, topic, qos.as_ref()) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(
                topic = label,
                error = %e,
                "DataReader::with_qos falhou; stream encerrado"
            );
            return None;
        }
    };
    let registration = match waitset.register(&reader) {
        Ok(r) => r,
        Err(e) => {
            tracing::error!(
                topic = label,
                error = %e,
                "waitset.register falhou; stream encerrado"
            );
            return None;
        }
    };
    Some((reader, registration))
}

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
    /// único do `DataSpace`. Setup EAGER via [`setup_stream_reader`]: o reader
    /// (com o perfil QoS do tópico) já existe antes do stream ser devolvido,
    /// para que um pump persistente esteja ativo antes do primeiro write em
    /// um tópico Volatile. Cada amostra alimenta o cache (upsert monotônico)
    /// e só as ACEITAS são entregues (RUST-CACHE-006).
    pub fn stream_tasks(&self) -> impl Stream<Item = cache::ArcTask> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "Tasks",
            &self.subscriber,
            &self.tasks_topic,
            &self.shared_waitset,
            qos::profiles::tasks(None),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return; // T-820-05/P1-7: DataSpace derrubado → fim gracioso
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(tasks) if !tasks.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
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
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(Tasks) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `AgentState` acordada por amostra (heartbeat dos agentes).
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_agent_states(&self) -> impl Stream<Item = cache::ArcAgentState> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "AgentRegistry",
            &self.subscriber,
            &self.agents_topic,
            &self.shared_waitset,
            qos::profiles::agent_registry(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(states) if !states.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for s in states {
                                let (state, accepted) = caches.upsert_agent(s);
                                if accepted {
                                    yield state;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(AgentRegistry) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `TaskOutput` acordada por amostra (chunks de inferência).
    /// Setup EAGER via [`setup_stream_reader`] (QoS do tópico, reader anexado
    /// antes de devolver o stream) para não perder amostras Volatile quando o
    /// pump do cliente ainda não foi polled. Só amostras aceitas pelo cache
    /// são entregues (T-820-06).
    pub fn stream_task_outputs(&self) -> impl Stream<Item = cache::ArcTaskOutput> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "TaskOutput",
            &self.subscriber,
            &self.outputs_topic,
            &self.shared_waitset,
            qos::profiles::task_output(None),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(outs) if !outs.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for o in outs {
                                let (output, accepted) = caches.push_output(o);
                                if accepted {
                                    yield output;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(TaskOutput) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `LLMInferenceRequest` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_llm_requests(&self) -> impl Stream<Item = cache::ArcLLMRequest> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "LLMRequest",
            &self.subscriber,
            &self.llm_request_topic,
            &self.shared_waitset,
            qos::profiles::llm(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(reqs) if !reqs.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for r in reqs {
                                let (req, accepted) = caches.upsert_llm_request(r);
                                if accepted {
                                    yield req;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(LLMRequest) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `LLMInferenceResult` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_llm_results(&self) -> impl Stream<Item = cache::ArcLLMResult> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "LLMResult",
            &self.subscriber,
            &self.llm_result_topic,
            &self.shared_waitset,
            qos::profiles::llm_result(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            // Padrão enable-antes-de-drenar (sem wakeup perdido): uma
            // notificação level-triggered cobre TUDO o que está no RHC —
            // drena até esvaziar. Notificações que chegarem durante o dreno
            // ficam capturadas no `Notified` habilitado e disparam um novo
            // ciclo de dreno. Antes (1 take por notificação), bursts
            // perdiam amostras no meio do stream (medido: 27–30/128).
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(results) if !results.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for r in results {
                                let (result, accepted) = caches.push_llm_result(r);
                                if accepted {
                                    yield result;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(LLMResult) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `LLMInferenceError` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_llm_errors(&self) -> impl Stream<Item = cache::ArcLLMError> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "LLMError",
            &self.subscriber,
            &self.llm_error_topic,
            &self.shared_waitset,
            qos::profiles::llm(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(errors) if !errors.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for e in errors {
                                let (error, accepted) = caches.upsert_llm_error(e);
                                if accepted {
                                    yield error;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(LLMError) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `ContextSnapshot` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_context_snapshots(
        &self,
    ) -> impl Stream<Item = cache::ArcContextSnapshot> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "ContextSnapshot",
            &self.subscriber,
            &self.context_snapshot_topic,
            &self.shared_waitset,
            qos::profiles::context_snapshot(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(snaps) if !snaps.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for s in snaps {
                                let (snap, accepted) = caches.upsert_context_snapshot(s);
                                if accepted {
                                    yield snap;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(ContextSnapshot) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `ContextUpdate` acordada por amostra. Setup EAGER
    /// (T-820-05/P1-3: o setup preguiçoso perdia a janela entre criar a
    /// stream e o primeiro poll; o tópico é TransientLocal desde
    /// T-850-03/D2 — paridade com o Python —, mas o stream de consumo
    /// vivo não deve depender do histórico retido) + filtro de admissão
    /// (T-820-06).
    pub fn stream_context_updates(&self) -> impl Stream<Item = cache::ArcContextUpdate> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "ContextUpdate",
            &self.subscriber,
            &self.context_update_topic,
            &self.shared_waitset,
            qos::profiles::context_update(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(updates) if !updates.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for u in updates {
                                let (update, accepted) = caches.push_context_update(u);
                                if accepted {
                                    yield update;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(ContextUpdate) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `ServerStatus` acordada por amostra (forma plural).
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_server_statuses(&self) -> impl Stream<Item = cache::ArcServerStatus> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "ServerStatus",
            &self.subscriber,
            &self.server_status_topic,
            &self.shared_waitset,
            qos::profiles::server_status(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(statuses) if !statuses.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for s in statuses {
                                let (status, accepted) = caches.upsert_server_status(s);
                                if accepted {
                                    yield status;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(ServerStatus) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `ToolCallRequest` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_tool_calls(&self) -> impl Stream<Item = cache::ArcToolCallRequest> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "ToolCall",
            &self.subscriber,
            &self.tool_call_topic,
            &self.shared_waitset,
            qos::profiles::tool_call(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(calls) if !calls.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for c in calls {
                                let (call, accepted) = caches.upsert_tool_call(c);
                                if accepted {
                                    yield call;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(ToolCall) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `ExecutionTraceEvent` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_execution_traces(
        &self,
    ) -> impl Stream<Item = cache::ArcExecutionTraceEvent> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "ExecutionTrace",
            &self.subscriber,
            &self.execution_trace_topic,
            &self.shared_waitset,
            qos::profiles::execution_trace(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(events) if !events.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for e in events {
                                let (event, accepted) = caches.push_execution_trace(e);
                                if accepted {
                                    yield event;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(ExecutionTrace) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `SecurityPolicySnapshot` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06);
    /// entrega o Arc do CACHE (não um segundo clone da amostra crua).
    pub fn stream_security_snapshots(
        &self,
    ) -> impl Stream<Item = cache::ArcSecurityPolicySnapshot> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "SecuritySnapshot",
            &self.subscriber,
            &self.security_snapshot_topic,
            &self.shared_waitset,
            qos::profiles::security_snapshot(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(snaps) if !snaps.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for s in snaps {
                                let (snap, accepted) = caches.upsert_security_snapshot(s);
                                if accepted {
                                    yield snap;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(SecuritySnapshot) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `SecurityPolicyUpdate` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_security_updates(
        &self,
    ) -> impl Stream<Item = cache::ArcSecurityPolicyUpdate> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "SecurityUpdate",
            &self.subscriber,
            &self.security_update_topic,
            &self.shared_waitset,
            qos::profiles::security_update(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(updates) if !updates.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for u in updates {
                                let (update, accepted) = caches.push_security_update(u);
                                if accepted {
                                    yield update;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(SecurityUpdate) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `QoSRoutingProfile` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_qos_routing(&self) -> impl Stream<Item = cache::ArcQoSRoutingProfile> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "QoSRouting",
            &self.subscriber,
            &self.qos_routing_topic,
            &self.shared_waitset,
            qos::profiles::qos_routing(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(profiles) if !profiles.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for p in profiles {
                                let (profile, accepted) = caches.upsert_qos_routing(p);
                                if accepted {
                                    yield profile;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(QoSRouting) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `QoSMetric` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_qos_metrics(&self) -> impl Stream<Item = cache::ArcQoSMetric> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "QoSMetric",
            &self.subscriber,
            &self.qos_metric_topic,
            &self.shared_waitset,
            qos::profiles::qos_metric(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(metrics) if !metrics.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for m in metrics {
                                let (metric, accepted) = caches.upsert_qos_metric(m);
                                if accepted {
                                    yield metric;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(QoSMetric) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `QoSViolation` acordada por amostra.
    /// Setup EAGER + QoS do tópico + filtro de admissão (T-820-05/T-820-06).
    pub fn stream_qos_violations(&self) -> impl Stream<Item = cache::ArcQoSViolation> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "QoSViolation",
            &self.subscriber,
            &self.qos_violation_topic,
            &self.shared_waitset,
            qos::profiles::qos_violation(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(violations) if !violations.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for v in violations {
                                let (violation, accepted) = caches.upsert_qos_violation(v);
                                if accepted {
                                    yield violation;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(QoSViolation) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `DiscoveryEvent` acordada por amostra. Setup EAGER
    /// (T-820-05/P1-3: tópico Volatile — o setup preguiçoso perdia a janela
    /// entre criar a stream e o primeiro poll) + filtro de admissão
    /// (T-820-06).
    pub fn stream_discovery_events(
        &self,
    ) -> impl Stream<Item = cache::ArcDiscoveryEvent> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "DiscoveryEvent",
            &self.subscriber,
            &self.discovery_event_topic,
            &self.shared_waitset,
            qos::profiles::qos_discovery(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(events) if !events.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for e in events {
                                let (event, accepted) = caches.upsert_discovery_event(e);
                                if accepted {
                                    yield event;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(DiscoveryEvent) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Streams `SystemMetrics` using the shared event-driven WaitSet (REQ-708).
    /// Setup EAGER via [`setup_stream_reader`] (QoS do tópico — T-820-05) +
    /// filtro de admissão (T-820-06).
    pub fn stream_system_metrics(&self) -> impl Stream<Item = cache::ArcSystemMetric> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "SystemMetrics",
            &self.subscriber,
            &self.system_metrics_topic,
            &self.shared_waitset,
            qos::profiles::system_metrics(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(metrics) if !metrics.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for metric in metrics {
                                let (m, accepted) = caches.upsert_system_metric(metric);
                                if accepted {
                                    yield m;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(error) => {
                            tracing::warn!(
                                %error,
                                backoff_ms = backoff.as_millis(),
                                "take_async(SystemMetrics) failed; retrying with growing backoff"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Streams `ServerStatus` using the shared event-driven WaitSet (REQ-708).
    /// Setup EAGER via [`setup_stream_reader`] (QoS do tópico — T-820-05) +
    /// filtro de admissão (T-820-06).
    pub fn stream_server_status(&self) -> impl Stream<Item = cache::ArcServerStatus> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "ServerStatus",
            &self.subscriber,
            &self.server_status_topic,
            &self.shared_waitset,
            qos::profiles::server_status(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(statuses) if !statuses.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for status in statuses {
                                let (s, accepted) = caches.upsert_server_status(status);
                                if accepted {
                                    yield s;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(error) => {
                            tracing::warn!(
                                %error,
                                backoff_ms = backoff.as_millis(),
                                "take_async(ServerStatus) failed; retrying with growing backoff"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
            }
        }
    }

    /// Stream de `StudioNodePresence` acordada por amostra (T-890, 19º
    /// tópico `Studio.NodePresence` — heartbeat das instalações do Studio).
    /// Setup EAGER + QoS do tópico + filtro de admissão
    /// (T-820-05/T-820-06).
    pub fn stream_studio_node_presences(
        &self,
    ) -> impl Stream<Item = cache::ArcStudioNodePresence> + 'static {
        let caches = self.caches();
        let setup = setup_stream_reader(
            "StudioNodePresence",
            &self.subscriber,
            &self.studio_node_presence_topic,
            &self.shared_waitset,
            qos::profiles::studio_node_presence(),
        );
        async_stream::stream! {
            let Some((reader, registration)) = setup else {
                return;
            };
            let mut backoff = STREAM_TAKE_BACKOFF_INITIAL;
            loop {
                if registration.is_shutdown() {
                    return;
                }
                let n = registration.notified();
                tokio::pin!(n);
                n.as_mut().enable();
                loop {
                    match reader.take_async().await {
                        Ok(presences) if !presences.is_empty() => {
                            backoff = STREAM_TAKE_BACKOFF_INITIAL;
                            for p in presences {
                                let (presence, accepted) = caches.upsert_studio_node_presence(p);
                                if accepted {
                                    yield presence;
                                }
                            }
                        }
                        Ok(_) => break,
                        Err(e) => {
                            tracing::warn!(
                                error = %e,
                                backoff_ms = backoff.as_millis(),
                                "take_async(StudioNodePresence) falhou; retry com backoff crescente"
                            );
                            tokio::time::sleep(backoff).await;
                            backoff = (backoff * 2).min(STREAM_TAKE_BACKOFF_MAX);
                            break;
                        }
                    }
                }
                if registration.is_shutdown() {
                    return;
                }
                n.await;
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
    /// Falível: criação de entidade DDS pode falhar — `Err` em vez de panic.
    pub fn agents_reader_with(
        &self,
        qos: &cyclonedds::Qos,
        listener: &cyclonedds::Listener,
    ) -> Result<DataReader<AgentState>, api::DataSpaceError> {
        DataReader::with_qos_and_listener(
            &self.subscriber,
            &self.agents_topic,
            Some(qos),
            Some(listener),
        )
        .map_err(err)
    }

    /// Reader de `TaskOutput` com QoS e listener custom (monitor/T-306).
    pub fn outputs_reader_with(
        &self,
        qos: &cyclonedds::Qos,
        listener: &cyclonedds::Listener,
    ) -> Result<DataReader<TaskOutput>, api::DataSpaceError> {
        DataReader::with_qos_and_listener(
            &self.subscriber,
            &self.outputs_topic,
            Some(qos),
            Some(listener),
        )
        .map_err(err)
    }

    /// Writer de `AgentRegistry` com QoS custom (testes do monitor).
    pub fn agents_writer_with(
        &self,
        qos: &cyclonedds::Qos,
    ) -> Result<DataWriter<AgentState>, api::DataSpaceError> {
        DataWriter::with_qos(&self.publisher, &self.agents_topic, Some(qos)).map_err(err)
    }

    /// Writer de `TaskOutput` com QoS custom (testes do monitor).
    pub fn outputs_writer_with(
        &self,
        qos: &cyclonedds::Qos,
    ) -> Result<DataWriter<TaskOutput>, api::DataSpaceError> {
        DataWriter::with_qos(&self.publisher, &self.outputs_topic, Some(qos)).map_err(err)
    }

    /// Writer de `Tasks` com QoS custom (ex.: papel cliente=10 para submissões
    /// da API — se fosse 200, os claims dos agentes perderiam a arbitragem).
    pub fn tasks_writer_with(
        &self,
        qos: &cyclonedds::Qos,
    ) -> Result<DataWriter<Task>, api::DataSpaceError> {
        DataWriter::with_qos(&self.publisher, &self.tasks_topic, Some(qos)).map_err(err)
    }
}

#[cfg(feature = "dds")]
impl DataSpace {
    /// Pool de escrita com writers dedicados (mesmos perfis/strength do DataSpace).
    /// Falível: repassa falha de spawn de `WriterPool::new`.
    pub fn new_writer_pool(
        &self,
        n_workers: usize,
        capacity: usize,
    ) -> Result<writer_pool::WriterPool, api::DataSpaceError> {
        let s = self.ownership_strength;
        let q_agents = qos::profiles::agent_registry().map_err(err)?;
        let q_outputs = qos::profiles::task_output(Some(s)).map_err(err)?;

        // Mesmo pool com força variada por slot que `DataSpace::new()` usa —
        // ver `build_tasks_writer_pool`. Sem isso, `WriteRequest::Task`
        // (hoje só exercido pelos testes de `writer_pool`) reintroduziria o
        // desbalanceamento de carga entre agentes corrigido nesta sessão,
        // caso algum refactor futuro passe a rotear o claim loop por aqui.
        let tw = build_tasks_writer_pool(&self.publisher, &self.tasks_topic, s)?;
        let aw = DataWriter::with_qos(&self.publisher, &self.agents_topic, Some(&q_agents))
            .map_err(err)?;
        let ow = DataWriter::with_qos(&self.publisher, &self.outputs_topic, Some(&q_outputs))
            .map_err(err)?;

        writer_pool::WriterPool::new(n_workers, capacity, writer_pool::make_write_fn(tw, aw, ow)?)
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

    /// Publica a Task via writer de strength do CLIENTE (10), sem assumir o
    /// ownership da instância — usado pelo reaper/reatribuição para não
    /// congelar a task (T-820-03; Exclusive Ownership: writers de strength
    /// maior tornam-se donos e impedem claims futuros).
    ///
    /// Contraste com `write_task`: aquela roteia pelo pool do papel (para o
    /// ORQUESTRADOR, strength 200 — a escrita de reatribuição por ali tornaria
    /// o orquestrador dono e os agentes, strength 100–163, nunca mais venceriam
    /// a arbitragem: a task morria em PENDING — P0-1 da revisão). Aqui o write
    /// sai pelo `tasks_writer_client` (strength 10), criado uma única vez no
    /// boot: agentes vivos sempre superam 10 no claim seguinte.
    ///
    /// CUIDADO (T-820-03, validado no E2E `t820_failover_*`): um write em
    /// strength 10 contra um dono VIVO de strength maior é DESCARTADO pelo RHC
    /// — inclusive o ASSIGNED de um agente morto cujo writer ainda respira
    /// (participant vazado/fechando, ou lease do DDSI não expirado). O
    /// ownership só é liberado pela destruição do writer do dono
    /// (`relinquish_ownership` no RHC); quem reatribui deve re-publicar até o
    /// mesh refletir o PENDING (republisher do orquestrador). Proteção contra
    /// write atrasado do agente morto vem da geração (`retry_count` no guard
    /// monotônico do cache — `cache::supersedes`).
    async fn write_task_without_ownership(&self, task: Task) -> Result<(), api::DataSpaceError> {
        // Sem write-through (mesma razão de write_task): o cache é alimentado
        // APENAS pelas streams (visão do mesh).
        self.tasks_writer_client.write(&task).map_err(err)
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

    fn subscribe_server_status(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = ServerStatus> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_server_status().map(|status| (*status).clone()))
    }

    async fn shutdown(&self) -> Result<(), api::DataSpaceError> {
        // T-820-05/P1-7: limpa os caches; o teardown real
        // (participant/waitset/streams) acontece no Drop — o Drop do
        // `SharedWaitSet` marca a flag de shutdown e acorda os registros, e
        // cada `stream_*` termina graciosamente ao observá-la
        // (`Registration::is_shutdown`).
        self.caches.clear_all();
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
            .map(|m| (*m).clone()))
    }

    fn subscribe_system_metrics(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = SystemMetric> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_system_metrics().map(|a| (*a).clone()))
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
            .map(|s| (*s).clone()))
    }

    fn subscribe_server_statuses(
        &self,
    ) -> std::pin::Pin<Box<dyn Stream<Item = ServerStatus> + Send>> {
        use futures::StreamExt;
        Box::pin(self.stream_server_statuses().map(|a| (*a).clone()))
    }

    // ── ToolCall methods ────────────────────────────────────────────────

    async fn write_tool_call(&self, call: ToolCallRequest) -> Result<(), api::DataSpaceError> {
        self.tool_call_writer.write(&call).map_err(err)
    }

    /// T-890-06: publica o pedido SEM ser dono da instância (strength
    /// CLIENTE) — quem põe `ToolCall.Request` não pode bloquear a evolução
    /// do gateway (mesma razão de `write_task_without_ownership`).
    async fn write_tool_call_without_ownership(
        &self,
        call: ToolCallRequest,
    ) -> Result<(), api::DataSpaceError> {
        self.tool_call_writer_client.write(&call).map_err(err)
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

#[cfg(test)]
mod tests {
    use super::fnv1a64;

    /// T-820-20: vetores de referência do FNV-1a 64-bit (RFC-ish/test vectors
    /// públicos) — travam o algoritmo contra regressão de implementação.
    #[test]
    fn fnv1a64_vetores_de_referencia() {
        assert_eq!(fnv1a64(b""), 0xcbf2_9ce4_8422_2325);
        assert_eq!(fnv1a64(b"a"), 0xaf63_dc4c_8601_ec8c);
        assert_eq!(fnv1a64(b"foobar"), 0x8594_4171_f739_67e8);
        assert_eq!(fnv1a64(b"hello"), 0xa430_d846_80aa_bd0b);
    }

    /// T-820-20: o invariante do roteamento — o MESMO task_id mapeia para o
    /// MESMO slot (função pura, sem estado/seed) e os slots são válidos.
    #[cfg(feature = "dds")]
    #[test]
    fn select_task_writer_slot_e_deterministico_por_task_id() {
        use super::select_task_writer_slot;
        for task_id in ["t-1", "pool-3-42", "c0ffee", "task-uuid-aleatório"] {
            let first = select_task_writer_slot(task_id, 64);
            for _ in 0..5 {
                assert_eq!(select_task_writer_slot(task_id, 64), first);
            }
            assert!(first < 64);
        }
        assert_eq!(select_task_writer_slot("qualquer", 1), 0);
        assert_eq!(select_task_writer_slot("qualquer", 0), 0);
    }
}
