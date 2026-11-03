//! Spike de interoperabilidade Rust↔Python↔C++ via DDS (Fase 0b).
//!
//! Perfis QoS que espelham o `dds_backend` Python (medidos via trace SEDP em
//! 2026-07-17 — ver specs/010-interop-spike/REPORT.md). Sem paridade de QoS
//! (em especial `Ownership=Exclusive` nos tópicos v4), o matching XTypes
//! reader↔writer não acontece.

#[cfg(feature = "dds")]
pub mod profiles {
    use cyclonedds::{DdsResult, Durability, History, Ownership, Qos, QosBuilder};

    /// Tópico `Tasks` — espelha o writer Python:
    /// Reliable(10s), TransientLocal, KeepLast(50), Exclusive, latency 50 ms, tprio 8,
    /// liveliness lease 10 s (definição de tópico idêntica à do peer — ktopic match).
    /// `strength`: ownership strength do writer (papel: cliente=10, agente=100, orq=200);
    /// readers devem passar `None`.
    pub fn tasks(strength: Option<i32>) -> DdsResult<Qos> {
        let mut b = QosBuilder::new()
            .reliability(cyclonedds::Reliability::Reliable, 10_000_000_000)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(50))
            .ownership(Ownership::Exclusive)
            .liveliness(cyclonedds::Liveliness::Automatic, 10_000_000_000)
            .latency_budget(50_000_000) // 50 ms em ns
            .transport_priority(8);
        if let Some(s) = strength {
            b = b.ownership_strength(s);
        }
        b.build()
    }

    /// Tópico `TaskOutput` — Reliable(10s), TransientLocal, KeepLast(64),
    /// Exclusive, deadline 10 s, liveliness lease **infinito** (definição do peer),
    /// latency 50 ms, tprio 8.
    pub fn task_output(strength: Option<i32>) -> DdsResult<Qos> {
        let mut b = QosBuilder::new()
            .reliability(cyclonedds::Reliability::Reliable, 10_000_000_000)
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(64))
            .ownership(Ownership::Exclusive)
            .deadline(10_000_000_000) // 10 s em ns
            .latency_budget(50_000_000)
            .transport_priority(8);
        if let Some(s) = strength {
            b = b.ownership_strength(s);
        }
        b.build()
    }

    /// Tópicos `LLM.*` (orchestrator::, keyless) — Reliable, TransientLocal,
    /// KeepLast(10), **Shared** (sem ownership exclusivo, ao contrário dos v4).
    pub fn llm() -> DdsResult<Qos> {
        QosBuilder::new()
            .reliable()
            .durability(Durability::TransientLocal)
            .history(History::KeepLast(10))
            .build()
    }
}
