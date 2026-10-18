//! Mapper de objetivo semântico de QoS → QoS de APLICAÇÃO (T-900-M4-01;
//! REQ-900-005/015; ADR-002).
//!
//! PURO: não recebe nem toca entidades DDS (writer/reader/ownership) — a QoS
//! estrutural dos endpoints da fase 900 é FIXA (contracts.md §"QoS por
//! entidade"); o objetivo do Laya mapeia para KNOBS DE APLICAÇÃO:
//!
//! | Objetivo | Efeito de aplicação (V1) |
//! |---|---|
//! | BALANCED | limites padrão autorizados do ingresso |
//! | CRITICAL | prioridade de admissão máxima + preservar deadline (sem RT hard) |
//! | STREAM_LIKE | streaming pedido + validação de chunks (sem mudar History/Deadline) |
//! | LOW_COST | só modelos com preço CONHECIDO dentro do orçamento |
//! | FAILOVER | retry dentro de max_attempts em agentes distintos (sem exactly-once) |
//!
//! Fail-closed: decisão inválida/custo desconhecido com orçamento estrito →
//! REJEITA com causa; indisponibilidade NUNCA amplia capacidade/permissão
//! (fallback só por template pré-autorizado E mais restritivo).
//!
//! Telemetria (M4-02/V07): `requested` (objetivo Laya), `accepted` (objetivo
//! efetivamente admitido) e `effective_config_hash` (hash dos knobs) são
//! DISTINTOS por construção — nunca alegar mudança nativa do DDS.

use crate::decision::Verdict;
use dds_contract::workflow::dds_llm_workflow_v1::WorkflowIntent;
use sha2::{Digest, Sha256};

/// Ordinais QosIntent.
pub const QI_BALANCED: i32 = 0;
pub const QI_CRITICAL: i32 = 1;
pub const QI_STREAM_LIKE: i32 = 2;
pub const QI_LOW_COST: i32 = 3;
pub const QI_FAILOVER: i32 = 4;

/// Preço por modelo, em microunidades da `budget_unit` por 1k tokens
/// gerados (tabela versionada por `policy_revision` — preço CONHECIDO).
#[derive(Debug, Clone, PartialEq)]
pub struct PriceTable {
    pub policy_revision: u32,
    /// model_id -> microunidades por 1k tokens de saída.
    pub per_1k_output: std::collections::BTreeMap<String, u64>,
}

/// Templates de fallback PRÉ-AUTORIZADOS (id -> objetivo + limites).
#[derive(Debug, Clone, PartialEq)]
pub struct FallbackTemplate {
    pub id: String,
    /// Objetivo MAIS restritivo aplicado no fallback.
    pub objective: i32,
    pub max_tokens: u32,
}

/// Política do mapper (versionada; thresholds não vêm de README).
#[derive(Debug, Clone, PartialEq)]
pub struct QosMapperPolicy {
    pub prices: PriceTable,
    /// Templates autorizados para fallback (vazio = fallback desligado:
    /// indisponibilidade REJEITA, nunca escolhe sozinho — contracts).
    pub fallback_templates: Vec<FallbackTemplate>,
    /// Modelo default do ingresso (BALANCED) — precisa ter preço conhecido.
    pub default_model: String,
    /// Modelos permitidos p/ LOW_COST (ordenados por preço asc).
    pub low_cost_models: Vec<String>,
}

impl Default for QosMapperPolicy {
    fn default() -> Self {
        Self {
            prices: PriceTable {
                policy_revision: 1,
                per_1k_output: [
                    ("gemma-4-12b-it", 100u64),
                    ("tiny-fast", 10),
                    ("free-local", 0),
                ]
                .into_iter()
                .map(|(k, v)| (k.to_string(), v))
                .collect(),
            },
            fallback_templates: vec![FallbackTemplate {
                id: "direct-summarize".into(),
                objective: QI_BALANCED,
                max_tokens: 256,
            }],
            default_model: "gemma-4-12b-it".into(),
            low_cost_models: vec!["tiny-fast".into(), "free-local".into()],
        }
    }
}

/// Knobs de aplicação efetivos (NÃO é QoS nativo DDS).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct EffectiveQos {
    /// Objetivo Laya pedido (ordinal).
    pub requested_objective: i32,
    /// Objetivo aceito após restrições (pode diferir no fallback).
    pub accepted_objective: i32,
    /// Prioridade de admissão (1/5/10 do validator).
    pub admission_priority: i32,
    /// Streaming pedido (validação de chunks no engine).
    pub streaming: bool,
    /// Retry dentro de max_attempts em agentes distintos.
    pub failover_retries: bool,
    /// Modelo autorizado (null = qualquer conhecido).
    pub model_constraint: Option<String>,
    /// Teto de tokens mantido do ingresso (nunca ampliado).
    pub max_tokens: u32,
}

impl EffectiveQos {
    /// Hash canônico da configuração EFETIVA (telemetria V07).
    pub fn config_hash(&self) -> String {
        let pre = format!(
            "{}|{}|{}|{}|{}|{}|{}",
            self.requested_objective,
            self.accepted_objective,
            self.admission_priority,
            self.streaming as u8,
            self.failover_retries as u8,
            self.model_constraint.as_deref().unwrap_or("-"),
            self.max_tokens
        );
        format!("{:x}", Sha256::digest(pre.as_bytes()))
    }
}

/// Causas de rejeição do mapper.
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum QosReject {
    #[error("custo desconhecido (modelo '{0}' sem preço na tabela rev {1}) com orçamento estrito")]
    UnknownPrice(String, u32),
    #[error("orçamento insuficiente: estimado {estimated} > teto {budget} microunidades")]
    OverBudget { estimated: u64, budget: u64 },
    #[error("orçamento zero só admite custo conhecido zero (estimado {0})")]
    NonZeroCostWithZeroBudget(u64),
    #[error("fallback '{0}' não está autorizado na política")]
    FallbackNotAuthorized(String),
    #[error("sem fallback autorizado e decisão indisponível: rejeita (fail-closed)")]
    NoFallbackAvailable,
    #[error("verdict não é Accept (decision indisponível/inválida)")]
    NotAccept,
}

/// Resultado do mapeamento.
#[derive(Debug, Clone, PartialEq)]
pub enum QosMapping {
    Applied(EffectiveQos),
    /// Rejeitado com causa tipada (telemetria registra a causa).
    Rejected(QosReject),
}

/// Estimativa de custo (LIMITE SUPERIOR): tokens * preço por 1k.
fn estimate_cost(model: &str, max_tokens: u32, prices: &PriceTable) -> Option<u64> {
    prices
        .per_1k_output
        .get(model)
        .map(|p| (u64::from(max_tokens) * p).div_ceil(1000))
}

/// Mapeia um `Verdict::Accept` + intenção → QoS de aplicação.
///
/// `max_tokens` da intenção é o TETO agregado do workflow (contracts): o
/// mapper nunca o amplia; LOW_COST exige preço CONHECIDO dentro do teto.
pub fn map(verdict: &Verdict, intent: &WorkflowIntent, policy: &QosMapperPolicy) -> QosMapping {
    let Verdict::Accept {
        qos_profile,
        priority,
        ..
    } = verdict
    else {
        return QosMapping::Rejected(QosReject::NotAccept);
    };

    map_objective(*qos_profile, *priority, intent, policy)
}

/// Mapeamento por objetivo (também usado pelo fallback de decisão ausente).
pub fn map_objective(
    objective: i32,
    priority: i32,
    intent: &WorkflowIntent,
    policy: &QosMapperPolicy,
) -> QosMapping {
    let budget = intent.max_cost_microunits;
    let max_tokens = intent.max_tokens;

    // LOW_COST: preço conhecido obrigatório quando há orçamento ESTRIITO
    // (zero ou qualquer teto): custo desconhecido não satisfaz orçamento.
    let (model_constraint, accepted_objective) = match objective {
        QI_LOW_COST => {
            // Menor custo CONHECIDO dentro do teto (c <= budget cobre o caso
            // de orçamento zero: só custo zero passa).
            let pick = policy
                .low_cost_models
                .iter()
                .filter_map(|m| {
                    estimate_cost(m, max_tokens, &policy.prices)
                        .filter(|c| *c <= budget)
                        .map(|c| (c, m.clone()))
                })
                .min_by_key(|(c, _)| *c)
                .map(|(_, m)| m);
            match pick {
                Some(m) => (Some(m), QI_LOW_COST),
                None => {
                    // sem modelo low-cost conhecido dentro do teto: custo
                    // desconhecido/estourado com orçamento estrito → rejeita
                    let any_known = policy
                        .low_cost_models
                        .iter()
                        .find(|m| estimate_cost(m, max_tokens, &policy.prices).is_some());
                    return QosMapping::Rejected(match any_known {
                        Some(m) => QosReject::OverBudget {
                            estimated: estimate_cost(m, max_tokens, &policy.prices).unwrap_or(0),
                            budget,
                        },
                        None => QosReject::UnknownPrice(
                            policy
                                .low_cost_models
                                .first()
                                .cloned()
                                .unwrap_or_else(|| "low-cost".into()),
                            policy.prices.policy_revision,
                        ),
                    });
                }
            }
        }
        _ => {
            // Objetivos não-LOW_COST usam o modelo default; orçamento ZERO só
            // admite custo ZERO CONHECIDO.
            if budget == 0 {
                match estimate_cost(&policy.default_model, max_tokens, &policy.prices) {
                    Some(0) => {}
                    Some(c) => {
                        return QosMapping::Rejected(QosReject::NonZeroCostWithZeroBudget(c))
                    }
                    None => {
                        return QosMapping::Rejected(QosReject::UnknownPrice(
                            policy.default_model.clone(),
                            policy.prices.policy_revision,
                        ))
                    }
                }
            }
            (None, objective)
        }
    };

    let effective = EffectiveQos {
        requested_objective: objective,
        accepted_objective,
        admission_priority: match accepted_objective {
            QI_CRITICAL => 10,
            _ => priority,
        },
        streaming: accepted_objective == QI_STREAM_LIKE,
        failover_retries: accepted_objective == QI_FAILOVER,
        model_constraint,
        max_tokens,
    };
    QosMapping::Applied(effective)
}

/// Fallback de DECISÃO INDISPONÍVEL (verdict Error/Reject): template
/// pré-autorizado e MAIS RESTRITIVO — nunca amplia capacidade/permissão.
pub fn map_fallback(intent: &WorkflowIntent, policy: &QosMapperPolicy) -> QosMapping {
    let requested = intent.fallback_template_id.to_string();
    if requested.is_empty() {
        // Fallback vazio = rejeição (contracts): nunca escolha automática.
        return QosMapping::Rejected(QosReject::NoFallbackAvailable);
    }
    let Some(tpl) = policy.fallback_templates.iter().find(|t| t.id == requested) else {
        return QosMapping::Rejected(QosReject::FallbackNotAuthorized(requested));
    };
    // Template é mais restritivo: objetivo e teto próprios (nunca > intent).
    let mut t_intent = intent.clone();
    t_intent.max_tokens = tpl.max_tokens.min(intent.max_tokens);
    match map_objective(tpl.objective, 5, &t_intent, policy) {
        QosMapping::Applied(mut e) => {
            // A decisão do Laya NUNCA chegou: o objetivo "solicitado" é
            // desconhecido (-1) — telemetria honesta do fallback.
            e.requested_objective = -1;
            QosMapping::Applied(e)
        }
        other => other,
    }
}

#[cfg(test)]
mod tests {
    #[cfg(not(feature = "dds"))]
    #[allow(dead_code)]
    pub(super) fn s(v: &str) -> String {
        v.to_string()
    }
    #[cfg(feature = "dds")]
    #[allow(dead_code)]
    pub(super) fn s(v: &str) -> cyclonedds::DdsString {
        v.try_into().expect("ascii válido")
    }

    use super::*;

    pub(super) fn prices() -> PriceTable {
        PriceTable {
            policy_revision: 7,
            per_1k_output: [
                ("gemma-4-12b-it", 100u64),
                ("tiny-fast", 10),
                ("free-local", 0),
            ]
            .into_iter()
            .map(|(k, v)| (k.to_string(), v))
            .collect(),
        }
    }

    pub(super) fn policy() -> QosMapperPolicy {
        QosMapperPolicy {
            prices: prices(),
            fallback_templates: vec![FallbackTemplate {
                id: "direct-summarize".into(),
                objective: QI_BALANCED,
                max_tokens: 256,
            }],
            default_model: "gemma-4-12b-it".into(),
            low_cost_models: vec!["tiny-fast".into(), "free-local".into()],
        }
    }

    pub(super) fn intent(max_tokens: u32, budget: u64, fallback: &str) -> WorkflowIntent {
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
            security_context_ref: s(""), // security_context_ref,
            policy_revision: 7,
            questions_revision: 1,
            capability_catalog_revision: 1,
            checkpoint_revision: s(""), // checkpoint_revision,
            max_tokens,
            max_cost_microunits: budget,
            budget_unit: s("MICRO_USD"),
            fallback_template_id: s(fallback),
            payload_hash: s(""), // payload_hash,
        }
    }

    #[test]
    fn balanced_maps_with_default_limits() {
        let m = map_objective(QI_BALANCED, 5, &intent(1000, 1_000_000, ""), &policy());
        assert!(matches!(m, QosMapping::Applied(e) if !e.streaming
            && !e.failover_retries && e.admission_priority == 5));
    }

    #[test]
    fn critical_and_stream_and_failover_flags() {
        let p = policy();
        let m = map_objective(QI_CRITICAL, 1, &intent(1000, 1_000_000, ""), &p);
        assert!(matches!(&m, QosMapping::Applied(e) if e.admission_priority == 10));
        let m = map_objective(QI_STREAM_LIKE, 5, &intent(1000, 1_000_000, ""), &p);
        assert!(matches!(&m, QosMapping::Applied(e) if e.streaming));
        let m = map_objective(QI_FAILOVER, 5, &intent(1000, 1_000_000, ""), &p);
        assert!(matches!(&m, QosMapping::Applied(e) if e.failover_retries));
    }

    #[test]
    fn low_cost_rejects_unknown_price_with_strict_budget() {
        // Sem opção low-cost dentro do teto (teto 100 < custo 10_000 do mais
        // barato conhecido na lista) → OverBudget, nunca custo desconhecido.
        let mut p = policy();
        p.low_cost_models = vec!["tiny-fast".to_string()];
        let m = map_objective(QI_LOW_COST, 5, &intent(1_000_000, 100, ""), &p);
        assert!(matches!(
            m,
            QosMapping::Rejected(QosReject::OverBudget {
                estimated: 10_000,
                budget: 100
            })
        ));
        // Lista SEM preço conhecido nenhum → UnknownPrice (fail-closed).
        let mut p2 = policy();
        p2.low_cost_models = vec!["misterioso".to_string()];
        let m2 = map_objective(QI_LOW_COST, 5, &intent(1000, 1_000_000, ""), &p2);
        assert!(matches!(
            m2,
            QosMapping::Rejected(QosReject::UnknownPrice(_, 7))
        ));
    }

    #[test]
    fn low_cost_picks_cheapest_within_budget() {
        let m = map_objective(QI_LOW_COST, 5, &intent(1000, 1_000_000, ""), &policy());
        assert!(
            matches!(&m, QosMapping::Applied(e) if e.model_constraint.as_deref() == Some("free-local"))
        );
    }

    #[test]
    fn zero_budget_requires_known_zero_cost() {
        // default tem preço != 0: orçamento zero rejeita
        let m = map_objective(QI_BALANCED, 5, &intent(1000, 0, ""), &policy());
        assert!(matches!(
            m,
            QosMapping::Rejected(QosReject::NonZeroCostWithZeroBudget(100))
        ));
        // LOW_COST com free-local (preço 0): ok
        let m = map_objective(QI_LOW_COST, 5, &intent(1000, 0, ""), &policy());
        assert!(
            matches!(&m, QosMapping::Applied(e) if e.model_constraint.as_deref() == Some("free-local"))
        );
    }

    #[test]
    fn fallback_template_authorized_and_restrictive() {
        let m = map_fallback(&intent(10_000, 1_000_000, "direct-summarize"), &policy());
        match &m {
            QosMapping::Applied(e) => {
                assert_eq!(e.accepted_objective, QI_BALANCED);
                assert_eq!(e.max_tokens, 256, "template nunca amplia o teto");
            }
            _ => panic!("esperava Applied"),
        }
    }

    #[test]
    fn fallback_unauthorized_or_empty_rejects() {
        let m = map_fallback(&intent(10, 1_000_000, "magic-template"), &policy());
        assert!(matches!(
            m,
            QosMapping::Rejected(QosReject::FallbackNotAuthorized(_))
        ));
        let m = map_fallback(&intent(10, 1_000_000, ""), &policy());
        assert!(matches!(
            m,
            QosMapping::Rejected(QosReject::NoFallbackAvailable)
        ));
    }

    #[test]
    fn config_hash_distinguishes_objectives() {
        let p = policy();
        let i = intent(1000, 1_000_000, "");
        let a = map_objective(QI_BALANCED, 5, &i, &p);
        let b = map_objective(QI_CRITICAL, 5, &i, &p);
        let (QosMapping::Applied(ea), QosMapping::Applied(eb)) = (a, b) else {
            panic!()
        };
        assert_ne!(ea.config_hash(), eb.config_hash());
        // requested != accepted distintos na telemetria
        assert_eq!(ea.requested_objective, ea.accepted_objective);
        assert_eq!(eb.accepted_objective, QI_CRITICAL);
    }
}

/// Registro de telemetria de QoS por workflow (M4-02/V07): solicitado,
/// aceito e hash da configuração EFETIVA são observáveis SEPARADAMENTE;
/// nunca alegar mudança de QoS nativo do DDS (a estrutura é fixa — ADR-002).
#[derive(Debug, Clone, PartialEq, serde::Serialize)]
pub struct QosTelemetryRecord {
    pub workflow_id: String,
    pub requested_objective: i32,
    pub accepted_objective: i32,
    pub effective_config_hash: String,
    pub fallback_reason: Option<String>,
    pub reject_reason: Option<String>,
    /// A QoS ESTRUTURAL dos endpoints não varia por requisição (constante).
    pub dds_structural_profile: &'static str,
}

/// Constante de telemetria: endpoints fixos (Reliable/TransientLocal/
/// KeepLast(1)/Shared — contracts.md §"QoS por entidade").
pub const DDS_STRUCTURAL_PROFILE: &str = "fixed:reliable+transientlocal+keeplast1+shared";

impl QosTelemetryRecord {
    /// Registro a partir de um mapeamento aplicado.
    pub fn applied(workflow_id: &str, e: &EffectiveQos) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            requested_objective: e.requested_objective,
            accepted_objective: e.accepted_objective,
            effective_config_hash: e.config_hash(),
            fallback_reason: None,
            reject_reason: None,
            dds_structural_profile: DDS_STRUCTURAL_PROFILE,
        }
    }

    /// Registro de rejeição (causa tipada vira telemetria — REQ-900-014).
    pub fn rejected(workflow_id: &str, r: &QosReject) -> Self {
        Self {
            workflow_id: workflow_id.to_string(),
            requested_objective: -1,
            accepted_objective: -1,
            effective_config_hash: String::new(),
            fallback_reason: None,
            reject_reason: Some(r.to_string()),
            dds_structural_profile: DDS_STRUCTURAL_PROFILE,
        }
    }
}

#[cfg(test)]
mod v07_tests {
    use super::super::qos_mapper::tests::{intent, policy};
    use super::tests::s;
    use super::*;

    /// V07 (parte in-memory): dois perfis SIMULTÂNEOS mapeiam
    /// independentemente — o mapper é puro (sem estado compartilhado, sem
    /// handles DDS); hashes distintos e telemetria separada por workflow.
    #[test]
    fn two_simultaneous_profiles_map_independently_v07() {
        let p = policy();
        let ia = intent(1000, 1_000_000, "");
        let mut ib = intent(1000, 1_000_000, "");
        ib.workflow_id = s("wf-b");

        let (QosMapping::Applied(ea), QosMapping::Applied(eb)) = (
            map_objective(QI_CRITICAL, 5, &ia, &p),
            map_objective(QI_STREAM_LIKE, 5, &ib, &p),
        ) else {
            panic!()
        };
        assert_ne!(ea.config_hash(), eb.config_hash());

        let ra = QosTelemetryRecord::applied("wf-a", &ea);
        let rb = QosTelemetryRecord::applied("wf-b", &eb);
        assert_ne!(ra.effective_config_hash, rb.effective_config_hash);
        // solicitado/aceito/efetivo são campos distintos e observáveis
        assert_eq!(ra.requested_objective, QI_CRITICAL);
        assert_eq!(ra.accepted_objective, QI_CRITICAL);
        assert_eq!(rb.requested_objective, QI_STREAM_LIKE);
        // estrutura DDS constante em ambos (não muda por perfil)
        assert_eq!(ra.dds_structural_profile, rb.dds_structural_profile);
        assert_eq!(ra.dds_structural_profile, DDS_STRUCTURAL_PROFILE);
    }

    /// Fallback muda o ACEITO (não o solicitado): telemetria distingue.
    #[test]
    fn fallback_telemetry_distinguishes_requested_from_accepted() {
        let p = policy();
        let i = intent(10_000, 1_000_000, "direct-summarize");
        // intenção PEDIA critical (chegou a pedir), mas decisão indisponível:
        // o registro de fallback carrega o template aplicado.
        let QosMapping::Applied(e) = map_fallback(&i, &p) else {
            panic!()
        };
        let rec = QosTelemetryRecord::applied("wf-x", &e);
        assert_eq!(rec.accepted_objective, QI_BALANCED);
        assert_ne!(rec.requested_objective, rec.accepted_objective);
    }

    /// Rejeição vira telemetria com causa (sem fallback silencioso).
    #[test]
    fn rejection_telemetry_carries_reason() {
        let p = policy();
        let i = intent(10, 1_000_000, "");
        let QosMapping::Rejected(r) = map_objective(
            QI_BALANCED,
            5,
            &WorkflowIntent {
                max_tokens: 1000,
                max_cost_microunits: 0,
                ..i.clone()
            },
            &p,
        ) else {
            panic!()
        };
        let rec = QosTelemetryRecord::rejected("wf-y", &r);
        assert!(rec.reject_reason.is_some());
        assert!(rec.reject_reason.as_deref().unwrap().contains("zero"));
    }
}
