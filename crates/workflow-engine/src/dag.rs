//! Validador de DAG / plano candidato (T-900-M5-02; REQ-900-009; ADR-004).
//!
//! ZERO Task publicada para plano inválido (regras do Prompt Master §7):
//! ciclo, aresta para nó inexistente, id duplicado, capability fora do
//! catálogo, modelo sem disponibilidade (V1: qualquer declarado é aceito —
//! disponibilidade real é checada no claim), dependência impossível,
//! limites (nós/profundidade/fan-out/tokens), prazo vencido, TOOL (até o
//! gate V10 — side effect sem idempotência provada), payload inválido,
//! referência de saída indisponível e ORÇAMENTO AGREGADO (reserva do
//! planner + Σ node.max_tokens × max_attempts ≤ intent.max_tokens — sem
//! reduzir tentativas/saídas silenciosamente).

use crate::capability_catalog::catalog_v1;
use dds_contract::workflow::dds_llm_workflow_v1::WorkflowIntent;
use thiserror::Error;

/// Limites V1 (contracts.md; proposta de engenharia).
pub const MAX_NODES: usize = 32;
pub const MAX_DEPTH: usize = 16;
pub const MAX_FANOUT: usize = 8;
pub const MAX_OUTPUT_BYTES: u64 = 1024 * 1024;

/// Candidato de nó (entrada do planner/template — NÃO é o tipo de tópico).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct CandidateNode {
    pub node_id: String,
    pub kind: NodeKindCand,
    pub required_capabilities: Vec<String>,
    pub model_id: String,
    pub target_agent_id: String,
    pub prompt_template: String,
    pub dependencies: Vec<String>,
    /// outputs que este nó DECLARA produzir (nomes).
    pub output_names: Vec<String>,
    /// inputs: (producer_node_id, output_name).
    pub inputs: Vec<(String, String)>,
    pub max_attempts: u32,
    pub max_tokens: u64,
    pub max_output_bytes: u64,
    pub deadline_ns: u64,
    pub priority: i32,
    pub side_effect_key: String,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq, serde::Deserialize)]
pub enum NodeKindCand {
    #[serde(rename = "inference")]
    Inference,
    #[serde(rename = "tool")]
    Tool,
}

/// Plano candidato (saída do planner LLM ou template direto).
#[derive(Debug, Clone, PartialEq, serde::Deserialize)]
pub struct PlanCandidate {
    pub planner_id: String,
    pub planner_revision: String,
    /// Reserva de tokens que o PRÓPRIO planner consumiu (entra no agregado).
    pub planner_tokens: u64,
    pub nodes: Vec<CandidateNode>,
}

#[derive(Debug, Clone, PartialEq, Error)]
pub enum DagError {
    #[error("sem nós")]
    Empty,
    #[error("id duplicado: {0}")]
    DuplicateId(String),
    #[error("dependência inexistente: {0}")]
    UnknownDependency(String),
    #[error("dependência duplicada em {0}")]
    DuplicateDependency(String),
    #[error("ciclo envolvendo {0}")]
    Cycle(String),
    #[error("capability fora do catálogo: {0}")]
    UnknownCapability(String),
    #[error("limite excedido: {0}")]
    LimitViolation(String),
    #[error("prazo vencido: nó {0}")]
    DeadlinePassed(String),
    #[error("kind TOOL rejeitado até o gate V10 (idempotência/autorização): {0}")]
    ToolRejected(String),
    #[error("prioridade fora da escala 1/5/10: nó {0} tem {1}")]
    BadPriority(String, i32),
    #[error("max_attempts fora de 1..3: nó {0} tem {1}")]
    BadAttempts(String, u32),
    #[error("input inválido: {producer} não declara a saída {output}")]
    InvalidInput { producer: String, output: String },
    #[error("orçamento agregado excedido: planner {planner} + Σ(nodes) {nodes} > teto {budget}")]
    AggregateBudget {
        planner: u64,
        nodes: u64,
        budget: u64,
    },
}

/// Valida o candidato. `now_ns` para prazos.
pub fn validate_candidate(
    cand: &PlanCandidate,
    intent: &WorkflowIntent,
    now_ns: u64,
) -> Result<(), DagError> {
    let catalog = catalog_v1();
    if cand.nodes.is_empty() {
        return Err(DagError::Empty);
    }
    if cand.nodes.len() > MAX_NODES {
        return Err(DagError::LimitViolation(format!(
            "{} nós > {MAX_NODES}",
            cand.nodes.len()
        )));
    }
    let by_id: std::collections::HashMap<&str, &CandidateNode> = {
        let mut m = std::collections::HashMap::new();
        for n in &cand.nodes {
            if m.insert(n.node_id.as_str(), n).is_some() {
                return Err(DagError::DuplicateId(n.node_id.clone()));
            }
        }
        m
    };

    let mut total_tokens = cand.planner_tokens;
    for n in &cand.nodes {
        if let Some(unknown) = catalog
            .unknown(n.required_capabilities.iter().map(|s| s.as_str()))
            .first()
        {
            return Err(DagError::UnknownCapability(unknown.to_string()));
        }
        if !matches!(n.priority, 1 | 5 | 10) {
            return Err(DagError::BadPriority(n.node_id.clone(), n.priority));
        }
        if !(1..=3).contains(&n.max_attempts) {
            return Err(DagError::BadAttempts(n.node_id.clone(), n.max_attempts));
        }
        if n.max_output_bytes > MAX_OUTPUT_BYTES {
            return Err(DagError::LimitViolation(format!(
                "max_output_bytes {} > 1 MiB em {}",
                n.max_output_bytes, n.node_id
            )));
        }
        if matches!(n.kind, NodeKindCand::Tool) {
            return Err(DagError::ToolRejected(n.node_id.clone()));
        }
        if n.deadline_ns > 0 && n.deadline_ns <= now_ns {
            return Err(DagError::DeadlinePassed(n.node_id.clone()));
        }
        if n.deadline_ns > 0 && intent.deadline_ns > 0 && n.deadline_ns > intent.deadline_ns {
            return Err(DagError::LimitViolation(format!(
                "prazo do nó {} > prazo do workflow",
                n.node_id
            )));
        }
        // dependências existem, sem duplicata, fan-out
        let mut seen = std::collections::HashSet::new();
        for d in &n.dependencies {
            if !by_id.contains_key(d.as_str()) {
                return Err(DagError::UnknownDependency(d.clone()));
            }
            if !seen.insert(d.clone()) {
                return Err(DagError::DuplicateDependency(n.node_id.clone()));
            }
        }
        if n.dependencies.len() > MAX_FANOUT {
            return Err(DagError::LimitViolation(format!(
                "fan-out {} > {MAX_FANOUT} em {}",
                n.dependencies.len(),
                n.node_id
            )));
        }
        // inputs referem saídas DECLARADAS de predecessores
        for (producer, output) in &n.inputs {
            let p = by_id
                .get(producer.as_str())
                .ok_or_else(|| DagError::InvalidInput {
                    producer: producer.clone(),
                    output: output.clone(),
                })?;
            if !p.output_names.iter().any(|o| o == output) {
                return Err(DagError::InvalidInput {
                    producer: producer.clone(),
                    output: output.clone(),
                });
            }
            if !n.dependencies.contains(producer) {
                return Err(DagError::InvalidInput {
                    producer: producer.clone(),
                    output: format!("{output} (não é dependência declarada)"),
                });
            }
        }
        total_tokens =
            total_tokens.saturating_add(n.max_tokens.saturating_mul(u64::from(n.max_attempts)));
    }
    if total_tokens > u64::from(intent.max_tokens) {
        return Err(DagError::AggregateBudget {
            planner: cand.planner_tokens,
            nodes: total_tokens - cand.planner_tokens,
            budget: u64::from(intent.max_tokens),
        });
    }

    // ciclo + profundidade: DFS com cores
    fn depth_first(
        id: &str,
        by_id: &std::collections::HashMap<&str, &CandidateNode>,
        state: &mut std::collections::HashMap<String, u8>,
        _depth: usize,
    ) -> Result<usize, DagError> {
        match state.get(id).copied() {
            Some(2) => return Ok(0), // já concluído
            Some(1) => return Err(DagError::Cycle(id.to_string())),
            _ => {}
        }
        state.insert(id.to_string(), 1);
        let mut max_child = 0usize;
        for d in &by_id.get(id).expect("id validado").dependencies {
            let d = depth_first(d, by_id, state, _depth + 1)?;
            max_child = max_child.max(d);
        }
        state.insert(id.to_string(), 2);
        Ok(max_child + 1)
    }
    let mut state = std::collections::HashMap::new();
    for n in &cand.nodes {
        let d = depth_first(&n.node_id, &by_id, &mut state, 0)?;
        if d > MAX_DEPTH {
            return Err(DagError::LimitViolation(format!(
                "profundidade {d} > {MAX_DEPTH} via {}",
                n.node_id
            )));
        }
    }
    Ok(())
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "dds"))]
    #[allow(dead_code)]
    fn s(v: &str) -> String {
        v.to_string()
    }
    #[cfg(feature = "dds")]
    #[allow(dead_code)]
    fn s(v: &str) -> cyclonedds::DdsString {
        v.try_into().expect("ascii válido")
    }

    use super::*;

    fn node(id: &str, deps: &[&str]) -> CandidateNode {
        CandidateNode {
            node_id: id.into(),
            kind: NodeKindCand::Inference,
            required_capabilities: vec!["summarization".to_string()],
            model_id: String::new(),
            target_agent_id: String::new(),
            prompt_template: String::new(),
            dependencies: deps.iter().map(|s| s.to_string()).collect(),
            output_names: vec!["out".to_string()],
            inputs: vec![],
            max_attempts: 1,
            max_tokens: 10,
            max_output_bytes: 1024,
            deadline_ns: 9_000_000_000_000,
            priority: 5,
            side_effect_key: String::new(),
        }
    }

    fn intent(max_tokens: u32) -> WorkflowIntent {
        WorkflowIntent {
            request_id: s("r"),
            workflow_id: s("w"),
            client_id: s("c"),
            schema_version: 1,
            intent_revision: 1,
            command: 0,
            expected_plan_revision: 0,
            prompt: s("p"),
            created_at_ns: 1,
            deadline_ns: 9_000_000_000_000,
            security_context_ref: s(""),
            policy_revision: 1,
            questions_revision: 1,
            capability_catalog_revision: 1,
            checkpoint_revision: s(""),
            max_tokens,
            max_cost_microunits: 1000,
            budget_unit: s("u"),
            fallback_template_id: s(""),
            payload_hash: s(""),
        }
    }

    fn cand(nodes: Vec<CandidateNode>) -> PlanCandidate {
        PlanCandidate {
            planner_id: "static-template".to_string(),
            planner_revision: "v1".to_string(),
            planner_tokens: 0,
            nodes,
        }
    }

    #[test]
    fn fork_join_valid() {
        let c = cand(vec![
            node("a", &[]),
            node("b1", &["a"]),
            node("b2", &["a"]),
            node("j", &["b1", "b2"]),
        ]);
        assert!(validate_candidate(&c, &intent(1000), 1).is_ok());
    }

    #[test]
    fn cycle_rejected_with_zero_tasks() {
        let c = cand(vec![node("a", &["b"]), node("b", &["a"])]);
        assert!(matches!(
            validate_candidate(&c, &intent(1000), 1),
            Err(DagError::Cycle(_))
        ));
    }

    #[test]
    fn unknown_dependency_rejected() {
        let c = cand(vec![node("a", &["fantasma"])]);
        assert!(matches!(
            validate_candidate(&c, &intent(1000), 1),
            Err(DagError::UnknownDependency(_))
        ));
    }

    #[test]
    fn unknown_capability_rejected() {
        let mut n = node("a", &[]);
        n.required_capabilities = vec!["magic".to_string()];
        assert!(matches!(
            validate_candidate(&cand(vec![n]), &intent(1000), 1),
            Err(DagError::UnknownCapability(_))
        ));
    }

    #[test]
    fn tool_rejected_until_gate() {
        let mut n = node("a", &[]);
        n.kind = NodeKindCand::Tool;
        assert!(matches!(
            validate_candidate(&cand(vec![n]), &intent(1000), 1),
            Err(DagError::ToolRejected(_))
        ));
    }

    #[test]
    fn aggregate_budget_two_valid_nodes_over_limit_rejected() {
        // dois nós individualmente ok cuja soma excede o teto (V08/V09)
        let mut n1 = node("a", &[]);
        n1.max_tokens = 60;
        n1.max_attempts = 2; // 120
        let mut n2 = node("b", &["a"]);
        n2.max_tokens = 60;
        n2.max_attempts = 2; // 120 → total 240 > 200
        let err = validate_candidate(&cand(vec![n1, n2]), &intent(200), 1).unwrap_err();
        assert!(matches!(
            err,
            DagError::AggregateBudget {
                nodes: 240,
                budget: 200,
                ..
            }
        ));
    }

    #[test]
    fn planner_tokens_count_toward_budget() {
        let c = PlanCandidate {
            planner_id: "llm-planner".to_string(),
            planner_revision: "v1".to_string(),
            planner_tokens: 150,
            nodes: vec![node("a", &[])], // 10
        };
        assert!(matches!(
            validate_candidate(&c, &intent(100), 1),
            Err(DagError::AggregateBudget {
                planner: 150,
                budget: 100,
                ..
            })
        ));
    }

    #[test]
    fn input_must_reference_declared_output_of_dependency() {
        let mut n = node("b", &["a"]);
        n.inputs = vec![("a".to_string(), "saida_inexistente".to_string())];
        assert!(matches!(
            validate_candidate(&cand(vec![node("a", &[]), n]), &intent(1000), 1),
            Err(DagError::InvalidInput { .. })
        ));
    }

    #[test]
    fn deadline_and_limits_enforced() {
        let mut n = node("a", &[]);
        n.deadline_ns = 1; // vencido p/ now=1? deadline <= now → sim
        assert!(matches!(
            validate_candidate(&cand(vec![n]), &intent(1000), 1),
            Err(DagError::DeadlinePassed(_))
        ));
        let mut n2 = node("a", &[]);
        n2.max_attempts = 4;
        assert!(matches!(
            validate_candidate(&cand(vec![n2]), &intent(1000), 1),
            Err(DagError::BadAttempts(_, 4))
        ));
    }
}
