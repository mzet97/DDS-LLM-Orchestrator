//! Perfis de QoS online/estrutural (REQ-004).
//!
//! Espelha `fuzzy_qos_manager/profile_mapper.py`. Separação honesta:
//! - **estrutural** (fixado na criação da entidade): Reliability, Durability,
//!   History, Ownership.kind, Liveliness
//! - **online** (mutável em runtime no CycloneDDS avaliado): TransportPriority,
//!   LatencyBudget, OwnershipStrength
//!
//! Deadline é uma política estrutural: é aplicado ao criar a entidade e exige
//! recriação para mudar. O valor `0` representa duração infinita e, portanto,
//! não é enviado ao `QosBuilder`.

use crate::profiles;

/// Políticas imutáveis após criar a entidade DDS.
#[derive(Debug, Clone, PartialEq)]
pub struct StructuralQos {
    pub reliability: ReliabilityKind,
    pub durability: DurabilityKind,
    pub history_kind: HistoryKind,
    pub history_depth: i32,
    pub ownership: OwnershipKind,
    pub liveliness: LivelinessKind,
    /// Lease de liveliness em segundos.
    pub liveliness_lease_s: f64,
    /// Deadline em segundos (0 = infinito). É aplicado na criação da entidade;
    /// mudanças exigem recriação.
    pub deadline_s: f64,
}

/// Knobs mutáveis em runtime (TransportPriority, LatencyBudget, OwnershipStrength).
#[derive(Debug, Clone, PartialEq)]
pub struct OnlineKnobs {
    pub transport_priority: i32,
    /// Latency budget em milissegundos (0 = zero budget).
    pub latency_budget_ms: f64,
    /// Strength default sugerida para o writer neste perfil (pode ser sobrescrita
    /// pelo papel em `roles::*`).
    pub ownership_strength: i32,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum ReliabilityKind {
    BestEffort,
    Reliable,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum DurabilityKind {
    Volatile,
    TransientLocal,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum HistoryKind {
    KeepLast,
    KeepAll,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum OwnershipKind {
    Shared,
    Exclusive,
}

#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum LivelinessKind {
    Automatic,
    ManualByParticipant,
    ManualByTopic,
}

/// Erro ao resolver um perfil desconhecido.
#[derive(Debug, Clone, PartialEq, Eq, thiserror::Error)]
#[error("unknown QoS profile: {0}")]
pub struct UnknownProfile(pub String);

/// Resolve um perfil fuzzy pelo nome canônico (REQ-004).
///
/// Valores idênticos a `QoSProfileMapper.MAPPING` em `profile_mapper.py`.
pub fn qos_profile(name: &str) -> Result<(StructuralQos, OnlineKnobs), UnknownProfile> {
    let pair = match name {
        "QoS_LowCost" => (
            StructuralQos {
                reliability: ReliabilityKind::BestEffort,
                durability: DurabilityKind::Volatile,
                history_kind: HistoryKind::KeepLast,
                history_depth: 1,
                ownership: OwnershipKind::Exclusive,
                liveliness: LivelinessKind::Automatic,
                liveliness_lease_s: 2.0,
                deadline_s: 0.0,
            },
            OnlineKnobs {
                transport_priority: 0,
                latency_budget_ms: 0.0,
                ownership_strength: 0,
            },
        ),
        "QoS_Balanced" => (
            StructuralQos {
                reliability: ReliabilityKind::Reliable,
                durability: DurabilityKind::Volatile,
                history_kind: HistoryKind::KeepLast,
                history_depth: 10,
                ownership: OwnershipKind::Exclusive,
                liveliness: LivelinessKind::Automatic,
                liveliness_lease_s: 5.0,
                deadline_s: 5.0,
            },
            OnlineKnobs {
                transport_priority: 1,
                latency_budget_ms: 0.0,
                ownership_strength: 0,
            },
        ),
        "QoS_Critical" => (
            StructuralQos {
                reliability: ReliabilityKind::Reliable,
                durability: DurabilityKind::TransientLocal,
                history_kind: HistoryKind::KeepLast,
                history_depth: 64,
                ownership: OwnershipKind::Exclusive,
                liveliness: LivelinessKind::Automatic,
                liveliness_lease_s: 10.0,
                deadline_s: 2.0,
            },
            OnlineKnobs {
                transport_priority: 2,
                latency_budget_ms: 0.0,
                ownership_strength: 0,
            },
        ),
        "QoS_Failover" => (
            StructuralQos {
                reliability: ReliabilityKind::Reliable,
                durability: DurabilityKind::TransientLocal,
                history_kind: HistoryKind::KeepLast,
                history_depth: 32,
                ownership: OwnershipKind::Shared,
                liveliness: LivelinessKind::Automatic,
                liveliness_lease_s: 1.0,
                deadline_s: 2.0,
            },
            OnlineKnobs {
                transport_priority: 2,
                latency_budget_ms: 0.0,
                ownership_strength: 0,
            },
        ),
        "QoS_StreamLike" => (
            StructuralQos {
                reliability: ReliabilityKind::BestEffort,
                durability: DurabilityKind::Volatile,
                history_kind: HistoryKind::KeepLast,
                history_depth: 1,
                ownership: OwnershipKind::Exclusive,
                liveliness: LivelinessKind::ManualByParticipant,
                // Paridade com profile_mapper.py: 1.0s (mais robusto que 0.5s sob GC).
                liveliness_lease_s: 1.0,
                deadline_s: 1.0,
            },
            OnlineKnobs {
                transport_priority: 3,
                latency_budget_ms: 0.0,
                ownership_strength: 0,
            },
        ),
        other => return Err(UnknownProfile(other.to_string())),
    };
    Ok(pair)
}

/// Todos os perfis canônicos resolvidos (ordem = `profiles::ALL`).
pub fn all_profiles() -> Vec<(&'static str, StructuralQos, OnlineKnobs)> {
    profiles::ALL
        .iter()
        .map(|n| {
            let (s, o) = qos_profile(n).expect("ALL profiles are known");
            (*n, s, o)
        })
        .collect()
}

#[cfg(feature = "dds")]
impl StructuralQos {
    /// Constrói um `QosBuilder` com as políticas estruturais.
    ///
    /// Deadline é aplicado na criação quando finito. Ownership strength e
    /// transport priority ficam nos knobs online.
    pub fn apply_to(&self, mut builder: cyclonedds::QosBuilder) -> cyclonedds::QosBuilder {
        use cyclonedds::{Durability, History, Liveliness, Ownership, Reliability};

        builder = match self.reliability {
            ReliabilityKind::BestEffort => builder.best_effort(),
            ReliabilityKind::Reliable => builder.reliable(),
        };
        builder = match self.durability {
            DurabilityKind::Volatile => builder.volatile(),
            DurabilityKind::TransientLocal => builder.transient_local(),
        };
        builder = match self.history_kind {
            HistoryKind::KeepLast => builder.keep_last(self.history_depth),
            HistoryKind::KeepAll => builder.keep_all(),
        };
        builder = match self.ownership {
            OwnershipKind::Shared => builder.ownership(Ownership::Shared),
            OwnershipKind::Exclusive => builder.ownership(Ownership::Exclusive),
        };
        let lease_ns = (self.liveliness_lease_s * 1_000_000_000.0) as i64;
        builder = match self.liveliness {
            LivelinessKind::Automatic => builder.liveliness(Liveliness::Automatic, lease_ns),
            LivelinessKind::ManualByParticipant => {
                builder.liveliness(Liveliness::ManualByParticipant, lease_ns)
            }
            LivelinessKind::ManualByTopic => {
                builder.liveliness(Liveliness::ManualByTopic, lease_ns)
            }
        };
        if self.deadline_s > 0.0 {
            let deadline_ns = (self.deadline_s * 1_000_000_000.0) as i64;
            builder = builder.deadline(deadline_ns);
        }
        let _ = (
            Durability::Volatile,
            History::KeepAll,
            Reliability::BestEffort,
        );
        builder
    }
}

#[cfg(feature = "dds")]
impl OnlineKnobs {
    /// Aplica knobs mutáveis em runtime a um `QosBuilder`.
    pub fn apply_to(&self, builder: cyclonedds::QosBuilder) -> cyclonedds::QosBuilder {
        let budget_ns = (self.latency_budget_ms * 1_000_000.0) as i64;
        builder
            .transport_priority(self.transport_priority)
            .latency_budget(budget_ns)
            .ownership_strength(self.ownership_strength)
    }
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn all_five_profiles_resolve() {
        for name in profiles::ALL {
            let (s, o) = qos_profile(name).expect(name);
            assert!(o.transport_priority >= 0);
            assert!(s.history_depth >= 1 || s.history_kind == HistoryKind::KeepAll);
        }
        assert_eq!(all_profiles().len(), 5);
    }

    #[test]
    fn stream_like_lease_is_one_second() {
        let (s, o) = qos_profile("QoS_StreamLike").unwrap();
        assert!((s.liveliness_lease_s - 1.0).abs() < f64::EPSILON);
        assert_eq!(o.transport_priority, 3);
        assert_eq!(s.liveliness, LivelinessKind::ManualByParticipant);
        assert_eq!(s.reliability, ReliabilityKind::BestEffort);
    }

    #[test]
    fn critical_is_reliable_transient_local() {
        let (s, o) = qos_profile("QoS_Critical").unwrap();
        assert_eq!(s.reliability, ReliabilityKind::Reliable);
        assert_eq!(s.durability, DurabilityKind::TransientLocal);
        assert_eq!(s.history_depth, 64);
        assert_eq!(o.transport_priority, 2);
    }

    #[test]
    fn failover_uses_shared_ownership() {
        let (s, _) = qos_profile("QoS_Failover").unwrap();
        assert_eq!(s.ownership, OwnershipKind::Shared);
        assert!((s.liveliness_lease_s - 1.0).abs() < f64::EPSILON);
    }

    #[test]
    fn unknown_profile_errors() {
        assert!(qos_profile("QoS_DoesNotExist").is_err());
    }

    #[test]
    fn online_knobs_are_the_mutable_set() {
        // Documentação viva: só estes três são mutáveis em runtime.
        let (_, o) = qos_profile("QoS_Balanced").unwrap();
        let _ = o.transport_priority;
        let _ = o.latency_budget_ms;
        let _ = o.ownership_strength;
    }

    #[cfg(feature = "dds")]
    #[test]
    fn finite_deadlines_are_applied_in_nanoseconds() {
        for (name, expected_ns) in [
            ("QoS_Balanced", 5_000_000_000),
            ("QoS_Critical", 2_000_000_000),
            ("QoS_Failover", 2_000_000_000),
            ("QoS_StreamLike", 1_000_000_000),
        ] {
            let (structural, _) = qos_profile(name).unwrap();
            let qos = structural
                .apply_to(cyclonedds::QosBuilder::new())
                .build()
                .unwrap();
            assert_eq!(qos.deadline().unwrap(), Some(expected_ns), "{name}");
        }
    }

    #[cfg(feature = "dds")]
    #[test]
    fn infinite_deadline_is_left_unset() {
        let (structural, _) = qos_profile("QoS_LowCost").unwrap();
        let qos = structural
            .apply_to(cyclonedds::QosBuilder::new())
            .build()
            .unwrap();
        assert_eq!(qos.deadline().unwrap(), None);
    }
}
