//! Perfis QoS por tópico — espelham o `dds_backend` Python (medidos via SEDP
//! em 2026-07-17, ver specs/010-interop-spike/REPORT.md §3).
//!
//! SEM paridade aqui o matching XTypes com a malha Python não acontece:
//! - `Tasks`/`TaskOutput` exigem `Ownership=Exclusive` (strength por papel);
//! - a definição de tópico (ktopic) compara reliability/liveliness/deadline —
//!   devem ser idênticos aos do peer;
//! - `AgentRegistry` é Shared + Liveliness ManualByTopic (heartbeat).

#[cfg(feature = "dds")]
pub mod profiles {
    use cyclonedds::{
        DdsResult, Durability, History, Liveliness, Ownership, Qos, QosBuilder, Reliability,
    };

    const TEN_S: i64 = 10_000_000_000;
    const FIVE_S: i64 = 5_000_000_000;
    const THIRTY_S: i64 = 30_000_000_000;
    const LATENCY_50MS: i64 = 50_000_000;

    /// `Tasks`: Reliable(10s), TransientLocal, KeepLast(50), Exclusive,
    /// liveliness automático lease 10 s, latency 50 ms, tprio 8.
    /// `strength`: papel do writer (cliente=10, agente=100, orq=200); readers: `None`.
    pub fn tasks(strength: Option<i32>) -> DdsResult<Qos> {
        let mut b = QosBuilder::new()
            .reliability(Reliability::Reliable, TEN_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(50))
            .ownership(Ownership::Exclusive)
            .liveliness(Liveliness::Automatic, TEN_S)
            .latency_budget(LATENCY_50MS)
            .transport_priority(8);
        if let Some(s) = strength {
            b = b.ownership_strength(s);
        }
        b.build()
    }

    /// `TaskOutput`: Reliable(10s), TransientLocal, KeepLast(64), Exclusive,
    /// deadline 10 s, liveliness automático lease ∞, latency 50 ms, tprio 8.
    pub fn task_output(strength: Option<i32>) -> DdsResult<Qos> {
        let mut b = QosBuilder::new()
            .reliability(Reliability::Reliable, TEN_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(64))
            .ownership(Ownership::Exclusive)
            .deadline(TEN_S)
            .latency_budget(LATENCY_50MS)
            .transport_priority(8);
        if let Some(s) = strength {
            b = b.ownership_strength(s);
        }
        b.build()
    }

    /// `AgentRegistry`: Reliable(10s), TransientLocal, KeepLast(1), **Shared**,
    /// deadline 30 s, Liveliness ManualByTopic lease 10 s, latency 50 ms, tprio 8.
    pub fn agent_registry() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, TEN_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(1))
            .ownership(Ownership::Shared)
            .deadline(THIRTY_S)
            .liveliness(Liveliness::ManualByTopic, TEN_S)
            .latency_budget(LATENCY_50MS)
            .transport_priority(8)
            .build()
    }

    /// `SystemMetrics`: BestEffort, Volatile, KeepLast(1), Shared.
    pub fn system_metrics() -> DdsResult<Qos> {
        QosBuilder::new()
            .best_effort()
            .durability(Durability::Volatile)
            .history(History::KeepLast(1))
            .build()
    }

    /// `QoS.Metric`: Reliable(5s), TransientLocal, KeepLast(100), tprio 7.
    /// Espelha `qos_qos_metric()` do `dds_data_space.py`.
    pub fn qos_metric() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, FIVE_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(100))
            .transport_priority(7)
            .build()
    }

    /// `QoS.Violation`: Reliable(5s), TransientLocal, KeepLast(1000), tprio 8.
    /// Espelha `qos_qos_violation()` do `dds_data_space.py`.
    pub fn qos_violation() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, FIVE_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(1000))
            .transport_priority(8)
            .build()
    }

    /// `QoS.Discovery`: Reliable(5s), Volatile, KeepLast(50), tprio 6.
    /// Espelha `qos_discovery_event()` do `dds_data_space.py`.
    pub fn qos_discovery() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, FIVE_S)
            .durability(Durability::Volatile)
            .history(History::KeepLast(50))
            .transport_priority(6)
            .build()
    }

    /// `Execution.Trace`: Reliable(10s), TransientLocal, KeepLast(256),
    /// Exclusive, tprio 5. Espelha `qos_execution_trace()` do `dds_data_space.py`.
    pub fn execution_trace() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, TEN_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(256))
            .ownership(Ownership::Exclusive)
            .transport_priority(5)
            .build()
    }

    /// `Tasks` com knobs online do decisor (REQ-405): mesma estrutura de
    /// `tasks()`, mas sobrescreve TransportPriority/OwnershipStrength.
    ///
    /// NOTA (limitação medida, 2026-07-18): `latency_budget` NÃO é mutável em
    /// runtime neste CycloneDDS — `dds_set_qos` com delta em LatencyBudget
    /// retorna `OUT_OF_MEMORY` (repro em `spike-interop::diag-knobs`). Por isso
    /// o campo é omitido aqui (herda o valor corrente do writer); apenas
    /// TransportPriority e OwnershipStrength são aplicados quentes.
    pub fn tasks_with_knobs(
        strength: Option<i32>,
        knobs: &dds_contract::qos::OnlineKnobs,
    ) -> DdsResult<Qos> {
        let s = knobs.ownership_strength;
        let effective = strength.or(Some(s));
        let mut b = QosBuilder::new()
            .reliability(Reliability::Reliable, TEN_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(50))
            .ownership(Ownership::Exclusive)
            .liveliness(Liveliness::Automatic, TEN_S)
            .transport_priority(knobs.transport_priority);
        if let Some(sv) = effective {
            b = b.ownership_strength(sv);
        }
        b.build()
    }

    /// Tópicos `LLM.*` (orchestrator::, keyless): Reliable(10s), TransientLocal,
    /// KeepLast(10), Shared.
    pub fn llm() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, TEN_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(10))
            .build()
    }

    /// `Context.Snapshot`: Reliable(10s), TransientLocal, KeepLast(1), Exclusive.
    pub fn context_snapshot() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, TEN_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(1))
            .ownership(Ownership::Exclusive)
            .build()
    }

    /// `Context.Update`: Reliable(10s), Volatile, KeepLast(10), Exclusive.
    pub fn context_update() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, TEN_S)
            .durability(Durability::Volatile)
            .history(History::KeepLast(10))
            .ownership(Ownership::Exclusive)
            .build()
    }

    /// `ToolCall.Request`: Reliable(10s), TransientLocal, KeepLast(5), Exclusive.
    pub fn tool_call() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, TEN_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(5))
            .ownership(Ownership::Exclusive)
            .build()
    }

    /// `Security.PolicySnapshot`/`Security.PolicyUpdate`: no Python ambos usam
    /// `qos_security_policy()` — Reliable(5s), TransientLocal, KeepLast(1),
    /// Exclusive, tprio 9. Mantidas duas funções pelos nomes semânticos.
    pub fn security_snapshot() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, FIVE_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(1))
            .ownership(Ownership::Exclusive)
            .transport_priority(9)
            .build()
    }

    /// `Security.PolicyUpdate`: mesmo perfil do Python (`qos_security_policy`).
    pub fn security_update() -> DdsResult<Qos> {
        security_snapshot()
    }

    /// `QoS.RoutingProfile`: Reliable(5s), TransientLocal, KeepLast(1), tprio 9.
    /// Espelha `qos_qos_routing_profile()` do `dds_data_space.py` (sem Ownership
    /// explícito → Shared, default).
    pub fn qos_routing() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliability(Reliability::Reliable, FIVE_S)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(1))
            .transport_priority(9)
            .build()
    }
}
