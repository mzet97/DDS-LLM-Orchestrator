// GERADO por m1_02_mock_gen.py a partir de /var/mnt/HD1TB/tese/src/rust/crates/dds-contract/idl/WorkflowPlanning.schema.json — não editar à mão.
// Mock do build sem `dds`; a paridade com o IDL é travada por teste.

/// Workflow.Intent: SUBMIT/CANCEL — ordinais canônicos (emenda 3: campos usam i32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum IntentCommand {
    IC_SUBMIT = 0,
    IC_CANCEL = 1
}

/// objetivo semântico de QoS (ADR-002) — ordinais canônicos (emenda 3: campos usam i32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum QosIntent {
    QI_BALANCED = 0,
    QI_CRITICAL = 1,
    QI_STREAM_LIKE = 2,
    QI_LOW_COST = 3,
    QI_FAILOVER = 4
}

/// OK/ERROR — ordinais canônicos (emenda 3: campos usam i32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum DecisionStatus {
    DS_OK = 0,
    DS_ERROR = 1
}

/// causas tipadas (ordem congelada) — ordinais canônicos (emenda 3: campos usam i32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum ReasonCode {
    RC_NONE = 0,
    RC_INVALID_INPUT = 1,
    RC_MODEL_UNAVAILABLE = 2,
    RC_TIMEOUT = 3,
    RC_SCHEMA_MISMATCH = 4,
    RC_POLICY_DENIED = 5,
    RC_LOW_CONFIDENCE = 6,
    RC_EXPIRED = 7,
    RC_CAPABILITY_UNAVAILABLE = 8,
    RC_OVERLOADED = 9,
    RC_CONFLICT = 10,
    RC_INVALID_PLAN = 11,
    RC_OUTPUT_INVALID = 12,
    RC_IN_DOUBT = 13,
    RC_CANCELLED = 14
}

/// INFERENCE/TOOL — ordinais canônicos (emenda 3: campos usam i32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum NodeKind {
    NK_INFERENCE = 0,
    NK_TOOL = 1
}

/// ciclo de vida do workflow — ordinais canônicos (emenda 3: campos usam i32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum WorkflowStatus {
    WS_ACCEPTED = 0,
    WS_WAITING_DECISION = 1,
    WS_WAITING_PLAN = 2,
    WS_RUNNING = 3,
    WS_DONE = 4,
    WS_FAILED = 5,
    WS_CANCELLED = 6,
    WS_IN_DOUBT = 7
}

/// estado por nó — ordinais canônicos (emenda 3: campos usam i32).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
#[repr(i32)]
pub enum NodeStatus {
    NS_BLOCKED = 0,
    NS_READY = 1,
    NS_PUBLISHED = 2,
    NS_ASSIGNED = 3,
    NS_RUNNING = 4,
    NS_DONE = 5,
    NS_FAILED = 6,
    NS_CANCELLED = 7,
    NS_IN_DOUBT = 8
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ProfileProbability {
    pub profile: i32,
    pub probability: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct CapabilityProbability {
    pub capability_id: String,  // bound 64
    pub probability: f64,
    pub answer_confidence: f64,
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct ToolVersion {
    pub tool_id: String,  // bound 64
    pub version: String,  // bound 64
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct InputRef {
    pub producer_node_id: String,  // bound 64
    pub output_name: String,  // bound 64
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct OutputRef {
    pub name: String,  // bound 64
    pub context_ref: String,  // bound 256
    pub content_hash: String,  // bound 64
    pub byte_count: u32,
    pub source_task_id: String,  // bound 70
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct PlanNode {
    pub node_id: String,  // bound 64
    pub kind: i32,
    pub required_capabilities: Vec<String>,  // bound seq 16
    pub model_id: String,  // bound 256
    pub target_agent_id: String,  // bound 64
    pub prompt_template: String,  // bound 8192
    pub dependencies: Vec<String>,  // bound seq 32
    pub inputs: Vec<InputRef>,  // bound seq 16
    pub output_names: Vec<String>,  // bound seq 8
    pub max_attempts: u16,
    pub deadline_ns: u64,
    pub max_tokens: u32,
    pub max_output_bytes: u32,
    pub security_context_ref: String,  // bound 256
    pub priority: i32,
    pub side_effect_key: String,  // bound 64
}

#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct NodeState {
    pub node_id: String,  // bound 64
    pub status: i32,
    pub attempt_id: u32,
    pub task_id: String,  // bound 70
    pub assigned_agent_id: String,  // bound 64
    pub ready: bool,
    pub started_at_ns: u64,
    pub completed_at_ns: u64,
    pub outputs: Vec<OutputRef>,  // bound seq 8
    pub reason_code: i32,
}

/// Chaves lógicas (IDL @key): request_id.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowIntent {
    pub request_id: String,  // bound 64
    pub workflow_id: String,  // bound 64
    pub client_id: String,  // bound 64
    pub schema_version: u16,
    pub intent_revision: u32,
    pub command: i32,
    pub expected_plan_revision: u32,
    pub prompt: String,  // bound 65536
    pub created_at_ns: u64,
    pub deadline_ns: u64,
    pub security_context_ref: String,  // bound 256
    pub policy_revision: u32,
    pub questions_revision: u32,
    pub capability_catalog_revision: u32,
    pub checkpoint_revision: String,  // bound 64
    pub max_tokens: u32,
    pub max_cost_microunits: u64,
    pub budget_unit: String,  // bound 64
    pub fallback_template_id: String,  // bound 64
    pub payload_hash: String,  // bound 64
}

/// Chaves lógicas (IDL @key): decision_id, request_id.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct RoutingDecision {
    pub request_id: String,  // bound 64
    pub decision_id: String,  // bound 64
    pub workflow_id: String,  // bound 64
    pub schema_version: u16,
    pub intent_revision: u32,
    pub producer_id: String,  // bound 64
    pub model_id: String,  // bound 256
    pub checkpoint_revision: String,  // bound 64
    pub questions_revision: u32,
    pub policy_revision: u32,
    pub capability_catalog_revision: u32,
    pub qos_profile: i32,
    pub qos_probabilities: Vec<ProfileProbability>,  // bound seq 5
    pub capability_probabilities: Vec<CapabilityProbability>,  // bound seq 16
    pub requires_planning_probability: f64,
    pub priority_score: f64,
    pub priority_probabilities: Vec<f64>,  // bound seq 3
    pub confidence: f64,
    pub answer_confidence: f64,
    pub calibration_revision: String,  // bound 64
    pub created_at_ns: u64,
    pub valid_until_ns: u64,
    pub decision_status: i32,
    pub reason_code: i32,
    pub evidence_digest: String,  // bound 64
    pub payload_hash: String,  // bound 64
}

/// Chaves lógicas (IDL @key): agent_id.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct AgentCapabilities {
    pub agent_id: String,  // bound 64
    pub schema_version: u16,
    pub announcement_revision: u32,
    pub capability_catalog_revision: u32,
    pub capabilities: Vec<String>,  // bound seq 64
    pub model_ids: Vec<String>,  // bound seq 16
    pub tool_versions: Vec<ToolVersion>,  // bound seq 32
    pub adaptive_protocol_version: u16,
    pub security_context_ref: String,  // bound 256
    pub created_at_ns: u64,
    pub valid_until_ns: u64,
}

/// Chaves lógicas (IDL @key): plan_revision, workflow_id.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowPlan {
    pub workflow_id: String,  // bound 64
    pub plan_revision: u32,
    pub schema_version: u16,
    pub request_id: String,  // bound 64
    pub intent_revision: u32,
    pub effective_decision_id: String,  // bound 64
    pub policy_revision: u32,
    pub capability_catalog_revision: u32,
    pub planner_id: String,  // bound 256
    pub planner_revision: String,  // bound 64
    pub created_at_ns: u64,
    pub deadline_ns: u64,
    pub qos_profile: i32,
    pub nodes: Vec<PlanNode>,  // bound seq 32
    pub plan_hash: String,  // bound 64
}

/// Chaves lógicas (IDL @key): plan_revision, workflow_id.
#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]
pub struct WorkflowState {
    pub workflow_id: String,  // bound 64
    pub plan_revision: u32,
    pub schema_version: u16,
    pub state_revision: u32,
    pub request_id: String,  // bound 64
    pub effective_decision_id: String,  // bound 64
    pub plan_hash: String,  // bound 64
    pub status: i32,
    pub reason_code: i32,
    pub nodes: Vec<NodeState>,  // bound seq 32
    pub created_at_ns: u64,
    pub updated_at_ns: u64,
    pub expires_at_ns: u64,
    pub cancelled_by_request_id: String,  // bound 64
    pub effective_qos_config_hash: String,  // bound 64
    pub fallback_template_id: String,  // bound 64
}
