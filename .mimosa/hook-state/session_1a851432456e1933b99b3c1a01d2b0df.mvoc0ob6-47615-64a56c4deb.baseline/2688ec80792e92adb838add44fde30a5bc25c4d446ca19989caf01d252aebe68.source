//! Validator de `Routing.Decision` (T-900-M2-02; REQ-900-004/015).
//!
//! A decisão do Laya é dado NÃO CONFIÁVEL até esta validação (D10). O
//! validator é puro (mocks do dds-contract, sem DDS) e decide:
//! - [`Verdict::Accept`] — previsão válida; deriva `required_capabilities`
//!   (limiar versionado) e prioridade (argmax LOW→NORMAL→HIGH → 1/5/10).
//! - [`Verdict::Error`] — decisão de ERRO bem formada (status DS_ERROR com
//!   reason ≠ NONE): legítima, sem previsão; intake/engine seguem política
//!   de fallback (M4/M5), nunca "materialização irrestrita".
//! - [`Verdict::Reject`] — malformada/não confiável: causa tipada.
//!
//! Regras do contracts.md §Routing.Decision; `shadow_only` marca decisões
//! sem `calibration_revision` (só observação até calibração válida).

use crate::capability_catalog::{catalog_v1, Catalog};
use dds_contract::workflow::dds_llm_workflow_v1::{RoutingDecision, WorkflowIntent};

/// Skew tolerado entre hosts com relógios sincronizados (contracts: 5 s).
pub const DEFAULT_SKEW_TOLERANCE_NS: u64 = 5_000_000_000;

/// Política versionada de admissão de decisões.
#[derive(Debug, Clone, PartialEq)]
pub struct DecisionPolicy {
    /// Revisão do catálogo que esta política entende (deve casar com a da
    /// intenção — sem seleção de revisão "mais recente").
    pub catalog_revision: u32,
    /// Probabilidade >= limiar torna a capacidade EXIGIDA (derivação Rust;
    /// a política é calibrável/versionada — thresholds não vêm do README).
    pub capability_threshold: f64,
    /// Gate de confiança: `Some(t)` + `enforce_confidence` rejeita abaixo de
    /// `t`. No observador M2 o gate é informativo (`enforce = false`).
    pub confidence_threshold: Option<f64>,
    pub enforce_confidence: bool,
    pub skew_tolerance_ns: u64,
    /// A3 (T-900): pin da calibração VÁLIDA (sha256 do payload do SDK).
    /// `Some(pin)` exige `decision.calibration_revision == pin` para
    /// `shadow_only=false`; sem pin, toda decisão aceita segue shadow
    /// (calibração ausente não ativa execução — contracts.md).
    pub calibration_pin: Option<String>,
}

impl Default for DecisionPolicy {
    fn default() -> Self {
        Self {
            catalog_revision: 1,
            capability_threshold: 0.5,
            confidence_threshold: None,
            enforce_confidence: false,
            skew_tolerance_ns: DEFAULT_SKEW_TOLERANCE_NS,
            calibration_pin: None,
        }
    }
}

/// Causas tipadas de rejeição (mapeadas para `ReasonCode` no intake M5).
#[derive(Debug, Clone, Copy, PartialEq, thiserror::Error)]
pub enum RejectReason {
    #[error("schema_version inválido")]
    SchemaVersion,
    #[error("correlação request/workflow/intent_revision não confere")]
    Correlation,
    #[error("revisões pinadas pela intenção não conferem (questions/policy/catalog/checkpoint)")]
    RevisionPin,
    #[error("janela de validade violada (valid_until > deadline+skew ou created > valid_until)")]
    ValidityWindow,
    #[error("status ERROR sem reason, ou OK com reason != NONE")]
    StatusReason,
    #[error("distribuição de QoS inválida (domínio/duplicata/soma/argmax)")]
    QosDistribution,
    #[error("probabilidades fora de [0,1] ou não finitas")]
    ProbabilityOutOfRange,
    #[error("prioridade inválida (score fora de [0,2] ou distribuição != 3 somando 1)")]
    PriorityInvalid,
    #[error("capacidade fora do catálogo {0}")]
    CapabilityUnknown(u32),
    #[error("capacidade duplicada na distribuição")]
    CapabilityDuplicate,
    #[error("confiança {0:.3} abaixo do limiar")]
    ConfidenceBelowThreshold(f64),
}

/// Resultado da validação de uma decisão contra a intenção que a originou.
#[derive(Debug, Clone, PartialEq)]
pub enum Verdict {
    Accept {
        /// Capacidades EXIGIDAS (prob >= limiar), na ordem do catálogo.
        required_capabilities: Vec<String>,
        /// Perfil escolhido (ordinal QosIntent 0..4).
        qos_profile: i32,
        /// Prioridade de aplicação (1/5/10) derivada de priority_probabilities.
        priority: i32,
        /// true quando `calibration_revision` vazia: aceita SÓ em shadow.
        shadow_only: bool,
        below_confidence_threshold: bool,
    },
    /// Decisão de erro bem formada (DS_ERROR + reason != NONE).
    Error { reason_code: i32 },
    Reject {
        reason: RejectReason,
        detail: String,
    },
}

fn finite_unit(p: f64) -> bool {
    p.is_finite() && (0.0..=1.0).contains(&p)
}

fn str_of(s: &impl std::fmt::Display) -> String {
    s.to_string()
}

/// Valida `decision` contra `intent` (origem da decisão).
///
/// `now_ns`: relógio do validador; `None` desativa checagem de expiração
/// (útil em testes determinísticos).
pub fn validate(
    decision: &RoutingDecision,
    intent: &WorkflowIntent,
    policy: &DecisionPolicy,
    now_ns: Option<u64>,
) -> Verdict {
    macro_rules! reject {
        ($r:expr, $($a:tt)*) => {
            return Verdict::Reject { reason: $r, detail: format!($($a)*) }
        };
    }

    // 1. Versão de schema (obrigatória 1 em V1 — emenda 2).
    if intent.schema_version != 1 || decision.schema_version != 1 {
        reject!(
            RejectReason::SchemaVersion,
            "intent.schema_version={} decision.schema_version={}",
            intent.schema_version,
            decision.schema_version
        );
    }

    // 2. Correlação.
    if str_of(&decision.request_id) != str_of(&intent.request_id)
        || str_of(&decision.workflow_id) != str_of(&intent.workflow_id)
        || decision.intent_revision != intent.intent_revision
    {
        reject!(
            RejectReason::Correlation,
            "decision ({}/{}/{}) != intent ({}/{}/{})",
            str_of(&decision.request_id),
            str_of(&decision.workflow_id),
            decision.intent_revision,
            str_of(&intent.request_id),
            str_of(&intent.workflow_id),
            intent.intent_revision
        );
    }

    // 3. Revisões pinadas pela intenção.
    if decision.questions_revision != intent.questions_revision
        || decision.policy_revision != intent.policy_revision
        || decision.capability_catalog_revision != intent.capability_catalog_revision
        || policy.catalog_revision != intent.capability_catalog_revision
    {
        reject!(
            RejectReason::RevisionPin,
            "questions {}/{} policy {}/{} catalog {}/{} (política entende rev {})",
            decision.questions_revision,
            intent.questions_revision,
            decision.policy_revision,
            intent.policy_revision,
            decision.capability_catalog_revision,
            intent.capability_catalog_revision,
            policy.catalog_revision
        );
    }
    let intent_checkpoint = str_of(&intent.checkpoint_revision);
    if !intent_checkpoint.is_empty() && str_of(&decision.checkpoint_revision) != intent_checkpoint {
        reject!(
            RejectReason::RevisionPin,
            "checkpoint_revision divergente do pin da intenção"
        );
    }

    // 4. Janela de validade.
    let skew = policy.skew_tolerance_ns;
    if decision.valid_until_ns > intent.deadline_ns.saturating_add(skew)
        || decision.created_at_ns > decision.valid_until_ns
    {
        reject!(
            RejectReason::ValidityWindow,
            "created {} valid_until {} deadline {} (skew {}ns)",
            decision.created_at_ns,
            decision.valid_until_ns,
            intent.deadline_ns,
            skew
        );
    }
    if let Some(now) = now_ns {
        if decision.valid_until_ns < now.saturating_sub(skew) {
            reject!(
                RejectReason::ValidityWindow,
                "decisão expirada: valid_until {} < now {}",
                decision.valid_until_ns,
                now
            );
        }
    }

    // 5. Status × reason.
    let ok = decision.decision_status == 0;
    if ok && decision.reason_code != 0 {
        reject!(
            RejectReason::StatusReason,
            "DS_OK com reason_code {}",
            decision.reason_code
        );
    }
    if !ok {
        if decision.reason_code == 0 {
            reject!(RejectReason::StatusReason, "DS_ERROR com reason_code NONE");
        }
        return Verdict::Error {
            reason_code: decision.reason_code,
        };
    }

    // 6. Distribuição de QoS: domínio 0..4, sem duplicata, soma 1, argmax.
    let profiles = decision.qos_probabilities.len();
    let mut seen = [false; 5];
    let mut total = 0.0f64;
    let mut best = (0usize, f64::NEG_INFINITY);
    for item in decision.qos_probabilities.iter() {
        let p = item.profile;
        if !(0..5).contains(&p) {
            reject!(
                RejectReason::QosDistribution,
                "perfil ordinal {p} fora de 0..4"
            );
        }
        if seen[p as usize] {
            reject!(
                RejectReason::QosDistribution,
                "perfil ordinal {p} duplicado"
            );
        }
        seen[p as usize] = true;
        if !finite_unit(item.probability) {
            reject!(
                RejectReason::ProbabilityOutOfRange,
                "qos prob {}",
                item.probability
            );
        }
        total += item.probability;
        // argmax com empate resolvido pela ORDEM declarada (primeiro maior).
        if item.probability > best.1 {
            best = (p as usize, item.probability);
        }
    }
    if profiles == 0 || (total - 1.0).abs() > 1e-6 {
        reject!(
            RejectReason::QosDistribution,
            "soma {total:.6} != 1 ({profiles} perfis)"
        );
    }
    if decision.qos_profile < 0 || decision.qos_profile > 4 {
        reject!(
            RejectReason::QosDistribution,
            "qos_profile ordinal {} fora de 0..4",
            decision.qos_profile
        );
    }
    if decision.qos_profile as usize != best.0 {
        reject!(
            RejectReason::QosDistribution,
            "qos_profile {} != argmax {}",
            decision.qos_profile,
            best.0
        );
    }

    // 7. Prioridade: score em [0,2]; distribuição de 3 somando 1.
    if !decision.priority_score.is_finite() || !(0.0..=2.0).contains(&decision.priority_score) {
        reject!(
            RejectReason::PriorityInvalid,
            "priority_score {} fora de [0,2]",
            decision.priority_score
        );
    }
    if decision.priority_probabilities.len() != 3 {
        reject!(
            RejectReason::PriorityInvalid,
            "priority_probabilities len {} != 3",
            decision.priority_probabilities.len()
        );
    }
    let mut prio_total = 0.0;
    for item in decision.priority_probabilities.iter() {
        if !finite_unit(*item) {
            reject!(RejectReason::ProbabilityOutOfRange, "priority prob {item}");
        }
        prio_total += *item;
    }
    if (prio_total - 1.0).abs() > 1e-6 {
        reject!(
            RejectReason::PriorityInvalid,
            "priority soma {prio_total:.6} != 1"
        );
    }

    // 8. requires_planning/confidences em [0,1].
    if !finite_unit(decision.requires_planning_probability)
        || !finite_unit(decision.confidence)
        || !finite_unit(decision.answer_confidence)
    {
        reject!(
            RejectReason::ProbabilityOutOfRange,
            "requires_planning/confidence fora de [0,1]"
        );
    }

    // 9. Capacidades: catálogo pinado, sem duplicata; deriva exigidas.
    let catalog: Catalog = catalog_v1();
    let labels: Vec<String> = decision
        .capability_probabilities
        .iter()
        .map(|c| str_of(&c.capability_id))
        .collect();
    if !catalog
        .unknown(labels.iter().map(|s| s.as_str()))
        .is_empty()
    {
        reject!(
            RejectReason::CapabilityUnknown(catalog.revision),
            "rótulos fora do catálogo rev {}: {:?}",
            catalog.revision,
            catalog.unknown(labels.iter().map(|s| s.as_str()))
        );
    }
    if !Catalog::duplicates(labels.iter().map(|s| s.as_str())).is_empty() {
        reject!(
            RejectReason::CapabilityDuplicate,
            "duplicatas em capability_probabilities"
        );
    }
    let mut required = Vec::new();
    for c in decision.capability_probabilities.iter() {
        if !finite_unit(c.probability) || !finite_unit(c.answer_confidence) {
            reject!(
                RejectReason::ProbabilityOutOfRange,
                "capability prob/confidence fora de [0,1]"
            );
        }
        if c.probability >= policy.capability_threshold {
            required.push(str_of(&c.capability_id));
        }
    }

    // 10. Gate de confiança (informativo no observador M2).
    let below = policy
        .confidence_threshold
        .map(|t| decision.confidence < t)
        .unwrap_or(false);
    if below && policy.enforce_confidence {
        reject!(
            RejectReason::ConfidenceBelowThreshold(decision.confidence),
            "confidence {} < limiar",
            decision.confidence
        );
    }

    // Prioridade 1/5/10: argmax LOW→NORMAL→HIGH (empate prefere o MENOR
    // índice = menor criticidade — "empate LOW→NORMAL→HIGH").
    let mut best_prio = 0usize;
    let mut best_p = f64::NEG_INFINITY;
    for (i, item) in decision.priority_probabilities.iter().enumerate() {
        if *item > best_p {
            best_p = *item;
            best_prio = i;
        }
    }
    let priority = [1i32, 5, 10][best_prio];

    // A3: pin de calibração — decisão com revisão casando sai de shadow.
    let shadow_only = match &policy.calibration_pin {
        Some(pin) => str_of(&decision.calibration_revision) != *pin,
        None => true, // sem calibração validada: permanece shadow
    };

    Verdict::Accept {
        required_capabilities: required,
        qos_profile: decision.qos_profile,
        priority,
        shadow_only,
        below_confidence_threshold: below,
    }
}
