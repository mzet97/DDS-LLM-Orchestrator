//! Elegibilidade por capacidades (T-900-M3-01; REQ-900-006; ADR-003).
//!
//! `Agent.Capabilities` é um ANÚNCIO substituível — não concede permissão
//! (D10). A elegibilidade combina, TODAS exigíveis:
//! 1. anúncio válido: catálogo pinado, rótulos conhecidos, revisão de
//!    protocolo 1, fresco (`valid_until_ns >= now`) e monotônico;
//! 2. registry (AgentState): saúde HEALTHY, slots livres, modelo compatível;
//! 3. requisitos da tarefa: capabilities exigidas ⊆ anunciadas (com a
//!    REVALIDAÇÃO no momento do claim — janela de revogação: um anúncio mais
//!    novo SEM a capacidade torna o agente inelegível imediatamente para
//!    novos claims; execução já em andamento segue o protocolo legado).
//!
//! `ModelSpecialization` NÃO é capability (eixo distinto — Prompt Master §3).

use crate::capability_catalog::{catalog_v1, Catalog};
use dds_contract::generated::dds_llm_orchestrator::AgentState;
use dds_contract::workflow::dds_llm_workflow_v1::AgentCapabilities;

/// Versão do protocolo adaptativo aceita em V1.
pub const ADAPTIVE_PROTOCOL_VERSION: u16 = 1;

/// Saúde exigida (ordinal AgentHealth: AH_OFFLINE=0, AH_DEGRADED=1, AH_HEALTHY=2).
const AH_HEALTHY: i32 = 2;

/// Causas de inelegibilidade (V05).
#[derive(Debug, Clone, PartialEq, thiserror::Error)]
pub enum NotEligible {
    #[error("sem anúncio de Agent.Capabilities")]
    NoAnnouncement,
    #[error("anúncio regrediu (revisão {received} < corrente {current})")]
    AnnouncementRegression { current: u32, received: u32 },
    #[error("adaptive_protocol_version {got} != {expected}")]
    ProtocolVersion { got: u16, expected: u16 },
    #[error("catálogo do anúncio rev {got} != pinado rev {expected}")]
    CatalogRevision { got: u32, expected: u32 },
    #[error("rótulos fora do catálogo: {0:?}")]
    UnknownLabels(Vec<String>),
    #[error("anúncio expirado (valid_until {valid_until_ns} < now {now_ns})")]
    Expired { valid_until_ns: u64, now_ns: u64 },
    #[error("capability {0} não anunciada")]
    MissingCapability(String),
    #[error("registry: saúde {health} != HEALTHY")]
    Unhealthy { health: i32 },
    #[error("registry: sem slots livres ({busy}/{total})")]
    NoSlots { busy: u32, total: u32 },
    #[error("registry: modelo requerido {0} indisponível no agente")]
    ModelUnavailable(String),
}

/// Verifica o ANÚNCIO em si (ordem/versão/frescor são checadas pelo espaço;
/// aqui: protocolo, catálogo, rótulos e expiração vs `now_ns`).
pub fn validate_announcement(
    caps: &AgentCapabilities,
    now_ns: u64,
    catalog: &Catalog,
) -> Result<(), NotEligible> {
    if caps.adaptive_protocol_version != ADAPTIVE_PROTOCOL_VERSION {
        return Err(NotEligible::ProtocolVersion {
            got: caps.adaptive_protocol_version,
            expected: ADAPTIVE_PROTOCOL_VERSION,
        });
    }
    if caps.capability_catalog_revision != catalog.revision {
        return Err(NotEligible::CatalogRevision {
            got: caps.capability_catalog_revision,
            expected: catalog.revision,
        });
    }
    let labels: Vec<String> = caps.capabilities.iter().map(|c| c.to_string()).collect();
    let unknown = catalog.unknown(labels.iter().map(|s| s.as_str()));
    if !unknown.is_empty() {
        return Err(NotEligible::UnknownLabels(
            unknown.into_iter().map(str::to_string).collect(),
        ));
    }
    if caps.valid_until_ns < now_ns {
        return Err(NotEligible::Expired {
            valid_until_ns: caps.valid_until_ns,
            now_ns,
        });
    }
    Ok(())
}

/// Elegibilidade completa para um conjunto de capacidades exigidas.
///
/// `required` vem do validator (prob >= limiar) ou do `PlanNode`.
/// `model_required`: `None` = qualquer; `Some(m)` exige `m` em `registry.model`
/// (ou vazio no registry = agente genérico NÃO satisfaz modelo específico).
pub fn is_eligible(
    announcement: Option<&AgentCapabilities>,
    registry: Option<&AgentState>,
    required: &[String],
    model_required: Option<&str>,
    now_ns: u64,
) -> Result<(), NotEligible> {
    let catalog = catalog_v1();
    let Some(caps) = announcement else {
        return Err(NotEligible::NoAnnouncement);
    };
    validate_announcement(caps, now_ns, &catalog)?;

    let announced: Vec<String> = caps.capabilities.iter().map(|c| c.to_string()).collect();
    for r in required {
        if !announced.iter().any(|a| a == r) {
            return Err(NotEligible::MissingCapability(r.clone()));
        }
    }

    // Registry: saúde/slots/modelo vêm de AgentState (autoridade operacional).
    if let Some(reg) = registry {
        if reg.health != AH_HEALTHY {
            return Err(NotEligible::Unhealthy { health: reg.health });
        }
        if reg.slots_busy >= reg.slots_total {
            return Err(NotEligible::NoSlots {
                busy: reg.slots_busy,
                total: reg.slots_total,
            });
        }
        if let Some(model) = model_required {
            let reg_model = reg.model.to_string();
            if reg_model != model {
                return Err(NotEligible::ModelUnavailable(model.to_string()));
            }
        }
    }
    Ok(())
}
