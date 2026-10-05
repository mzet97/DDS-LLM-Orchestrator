//! Paridade mock↔IDL (T-820-02/06, P1-5 da revisão 2026-10-04).
//!
//! Os mocks `dds_contract::generated::*` (`#[cfg(not(feature = "dds"))]`)
//! são cópias manuais dos tipos do IDL — o histórico já teve um bug M2 (mock
//! sem `requester_id` quebrando os testes com a feature `dds` ligada). Este
//! gate compara, para cada um dos 19 tipos canônicos, a lista de campos do
//! mock (via serde_json sobre `Default::default()`) contra a lista de campos
//! extraída DIRETO dos `.idl` da crate (`idl/OrchestratorV4.idl` +
//! `idl/OrchestratorDDS.idl`, a renderização mecânica do contrato — Entendimento
//! §§11–12), com um parser mínimo de struct IDL.
//!
//! **Risco residual (documentado):** os mocks só existem sem a feature `dds`
//! e, por isso, este gate roda em `cargo test -p dds-contract` (sem feature) —
//! que é o gate padrão do workspace (`cargo test --workspace`). Com
//! `--features dds` o módulo de mocks não é compilado e este arquivo é
//! cfg'd-out inteiro: não há mock para checar. Comparar mock↔gerado em
//! runtime não é possível: os tipos gerados não expõem enumeração de campos
//! nem Serialize (só `DdsType`/XCDR). A checagem contra o IDL pega o mesmo
//! drift (o gerado vem do IDL via `cyclonedds-idlc`), restando só o risco de
//! drift idlc↔IDL, coberto pelo compilador DDS real e pelos testes
//! `wire_typenames_match_idl_modules`/`v4_keys_match_pragma_keylist`.
#![cfg(not(feature = "dds"))]

use dds_contract::generated::dds_llm_orchestrator::{
    AgentState, ContextSnapshot, ContextUpdate, DiscoveryEvent, ExecutionTraceEvent, QoSMetric,
    QoSRoutingProfile, QoSViolation, SecurityPolicySnapshot, SecurityPolicyUpdate,
    StudioNodePresence, SystemMetric, Task, TaskOutput, ToolCallRequest,
};
use dds_contract::generated::orchestrator::{
    LLMInferenceError, LLMInferenceRequest, LLMInferenceResult, ServerStatus,
};
use dds_contract::topics;

/// Extrai os nomes de campos da struct `struct_name` de um arquivo IDL.
///
/// Mesma regra de `build.rs::field_name_from_decl`: ignora comentários `//`,
/// aceita apenas linhas de declaração terminadas em `;` e toma o último
/// token como nome do campo (o tipo pode ser multi-palavra, ex. `unsigned
/// long long`).
fn idl_struct_fields(idl: &str, struct_name: &str) -> Vec<String> {
    let needle = format!("struct {struct_name} {{");
    let start = idl
        .find(&needle)
        .unwrap_or_else(|| panic!("struct {struct_name} não encontrada no IDL"));
    let body_start = start + needle.len();
    let body_end = idl[body_start..]
        .find("};")
        .unwrap_or_else(|| panic!("struct {struct_name} sem fechamento '}};'"))
        + body_start;
    let body = &idl[body_start..body_end];
    let mut fields = Vec::new();
    for line in body.lines() {
        let decl = line.split("//").next().unwrap_or("").trim();
        if !decl.ends_with(';') {
            continue;
        }
        let no_semi = decl.trim_end_matches(';').trim();
        let name = no_semi
            .split_whitespace()
            .last()
            .unwrap_or_else(|| panic!("declaração sem campo: '{decl}'"));
        fields.push(name.to_string());
    }
    assert!(
        !fields.is_empty(),
        "struct {struct_name}: nenhum campo parseado (parser quebrou?)"
    );
    fields
}

/// Compara o CONJUNTO de campos do mock (serde_json sobre o default) com o
/// conjunto extraído do IDL.
///
/// Nota: a comparação é por conjunto (serde_json ordena as chaves do objeto)
/// e isso basta: o mock nunca é serializado em CDR — a ORDEM do wire é
/// definida pelo IDL e vale para os tipos gerados (checados em
/// `roundtrip_*`/`wire_typenames_*` com a feature `dds`). Campo a mais,
/// a menos ou renomeado no mock — o drift que já quebrou o build uma vez
/// (bug M2) — é exatamente o que este gate pega.
fn assert_mock_fields_match_idl<T: serde::Serialize + Default>(
    idl: &str,
    struct_name: &str,
    struct_label: &str,
) {
    let expected = idl_struct_fields(idl, struct_name);
    let mut sorted_expected = expected.clone();
    sorted_expected.sort();
    let value = serde_json::to_value(T::default())
        .unwrap_or_else(|e| panic!("serialize default de {struct_label}: {e}"));
    let map = value
        .as_object()
        .unwrap_or_else(|| panic!("{struct_label}: serde_json não devolveu objeto"));
    let mut actual: Vec<String> = map.keys().cloned().collect();
    actual.sort();
    assert_eq!(
        actual, sorted_expected,
        "mock {struct_label} divergiu do IDL (canônico): corrige o mock, não o IDL"
    );
}

#[test]
fn mocks_batem_com_os_19_tipos_do_idl() {
    // Sanidade do inventário: 19 tópicos canônicos (Entendimento §11 + 19º
    // `Studio.NodePresence` aprovado em 2026-10-05 — T-890).
    assert_eq!(topics::ALL.len(), 19);

    let v4 = include_str!("../idl/OrchestratorV4.idl");
    let dds = include_str!("../idl/OrchestratorDDS.idl");

    // OrchestratorDDS.idl → módulo `orchestrator` (4 tipos, keyless)
    assert_mock_fields_match_idl::<LLMInferenceRequest>(
        dds,
        "LLMInferenceRequest",
        "orchestrator::LLMInferenceRequest",
    );
    assert_mock_fields_match_idl::<LLMInferenceResult>(
        dds,
        "LLMInferenceResult",
        "orchestrator::LLMInferenceResult",
    );
    assert_mock_fields_match_idl::<LLMInferenceError>(
        dds,
        "LLMInferenceError",
        "orchestrator::LLMInferenceError",
    );
    assert_mock_fields_match_idl::<ServerStatus>(dds, "ServerStatus", "orchestrator::ServerStatus");

    // OrchestratorV4.idl → módulo `dds_llm_orchestrator` (15 tipos)
    assert_mock_fields_match_idl::<Task>(v4, "Task", "dds_llm_orchestrator::Task");
    assert_mock_fields_match_idl::<AgentState>(
        v4,
        "AgentState",
        "dds_llm_orchestrator::AgentState",
    );
    assert_mock_fields_match_idl::<TaskOutput>(
        v4,
        "TaskOutput",
        "dds_llm_orchestrator::TaskOutput",
    );
    assert_mock_fields_match_idl::<SystemMetric>(
        v4,
        "SystemMetric",
        "dds_llm_orchestrator::SystemMetric",
    );
    assert_mock_fields_match_idl::<QoSRoutingProfile>(
        v4,
        "QoSRoutingProfile",
        "dds_llm_orchestrator::QoSRoutingProfile",
    );
    assert_mock_fields_match_idl::<ContextSnapshot>(
        v4,
        "ContextSnapshot",
        "dds_llm_orchestrator::ContextSnapshot",
    );
    assert_mock_fields_match_idl::<ContextUpdate>(
        v4,
        "ContextUpdate",
        "dds_llm_orchestrator::ContextUpdate",
    );
    assert_mock_fields_match_idl::<ToolCallRequest>(
        v4,
        "ToolCallRequest",
        "dds_llm_orchestrator::ToolCallRequest",
    );
    assert_mock_fields_match_idl::<ExecutionTraceEvent>(
        v4,
        "ExecutionTraceEvent",
        "dds_llm_orchestrator::ExecutionTraceEvent",
    );
    assert_mock_fields_match_idl::<SecurityPolicySnapshot>(
        v4,
        "SecurityPolicySnapshot",
        "dds_llm_orchestrator::SecurityPolicySnapshot",
    );
    assert_mock_fields_match_idl::<SecurityPolicyUpdate>(
        v4,
        "SecurityPolicyUpdate",
        "dds_llm_orchestrator::SecurityPolicyUpdate",
    );
    assert_mock_fields_match_idl::<QoSMetric>(v4, "QoSMetric", "dds_llm_orchestrator::QoSMetric");
    assert_mock_fields_match_idl::<QoSViolation>(
        v4,
        "QoSViolation",
        "dds_llm_orchestrator::QoSViolation",
    );
    // T-890: 19º tópico (OrchestratorV4.idl → módulo `dds_llm_orchestrator`).
    assert_mock_fields_match_idl::<StudioNodePresence>(
        v4,
        "StudioNodePresence",
        "dds_llm_orchestrator::StudioNodePresence",
    );
    assert_mock_fields_match_idl::<DiscoveryEvent>(
        v4,
        "DiscoveryEvent",
        "dds_llm_orchestrator::DiscoveryEvent",
    );
}
