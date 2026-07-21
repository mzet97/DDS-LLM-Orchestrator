//! Harness de 5 braços (T-505, REQ-505): static / zadeh / fcm / fcm-dhl / nfcm
//! sobre os cenários canônicos — tabela por cenário + latência por decisão.
//!
//! Só métricas LOCAIS (latência por decide, medida aqui). Nenhum número de cluster
//! é inventado: para comparação de QoS sistêmico, usar o harness de benchmarks (WF-8).
//!
//! Rode com: `cargo run -p qos-nfcm --example five_arms --release`

use qos_nfcm::decider::{QoSMetrics, QosDecider, StaticDecider};
use qos_nfcm::fcm::{FcmDecider, FcmDhlDecider};
use qos_nfcm::zadeh::ZadehDecider;
use qos_nfcm::{Nfcm, QoSProfile};
use std::sync::Arc;
use std::time::Instant;

const KEYS: [&str; 8] = [
    "urgency",
    "deadline_pressure",
    "recent_latency",
    "agent_load",
    "error_rate",
    "historical_confidence",
    "estimated_complexity",
    "streaming_need",
];

const SCENARIOS: [(&str, [f64; 8]); 6] = [
    ("ocioso", [0.10, 0.05, 0.15, 0.10, 0.05, 0.90, 0.20, 0.05]),
    ("urgencia", [0.95, 0.90, 0.30, 0.35, 0.10, 0.85, 0.50, 0.10]),
    (
        "degradado",
        [0.60, 0.40, 0.85, 0.80, 0.90, 0.20, 0.50, 0.10],
    ),
    (
        "streaming",
        [0.50, 0.30, 0.40, 0.45, 0.10, 0.80, 0.40, 0.92],
    ),
    ("lowcost", [0.05, 0.10, 0.20, 0.15, 0.05, 0.70, 0.05, 0.05]),
    ("critico", [0.90, 0.95, 0.50, 0.40, 0.05, 0.80, 0.30, 0.20]),
];

fn qm(v: &[f64; 8]) -> QoSMetrics {
    QoSMetrics {
        urgency: v[0],
        deadline_pressure: v[1],
        recent_latency: v[2],
        agent_load: v[3],
        error_rate: v[4],
        historical_confidence: v[5],
        estimated_complexity: v[6],
        streaming_need: v[7],
    }
}

fn short(p: &QoSProfile) -> &'static str {
    match p {
        QoSProfile::Critical => "Critical",
        QoSProfile::Failover => "Failover",
        QoSProfile::StreamLike => "StreamLike",
        QoSProfile::LowCost => "LowCost",
        QoSProfile::Balanced => "Balanced",
    }
}

fn main() {
    let arms: Vec<(&str, Arc<dyn QosDecider>)> = vec![
        ("static", Arc::new(StaticDecider::new(QoSProfile::Balanced))),
        ("zadeh", Arc::new(ZadehDecider::new())),
        ("fcm", Arc::new(FcmDecider::new())),
        ("fcm-dhl", Arc::new(FcmDhlDecider::default())),
        ("nfcm", Arc::new(Nfcm::qos_default())),
    ];

    println!("\n=== 5 braços × cenários canônicos (perfil [conf] | ns/decide) ===\n");
    let header = format!(
        "{:<12} | {:<18} | {:<18} | {:<18} | {:<18} | {:<18}",
        "cenário", "static", "zadeh", "fcm", "fcm-dhl", "nfcm"
    );
    println!("{header}");
    println!("{}", "-".repeat(header.len()));

    for (name, vals) in &SCENARIOS {
        let mut row = format!("{name:<12} | ");
        let mut cells = Vec::new();
        for (_, decider) in &arms {
            let m = qm(vals);
            let t0 = Instant::now();
            let d = decider.decide(&m);
            let ns = t0.elapsed().as_nanos();
            cells.push(format!(
                "{:<18}",
                format!("{} [{:.2}] {}ns", short(&d.profile), d.confidence, ns)
            ));
        }
        row.push_str(&cells.join(" | "));
        println!("{row}");
    }

    // Divergências: cenários onde os braços discordam entre si
    println!("\n=== divergências (braços discordam no mesmo cenário) ===");
    for (name, vals) in &SCENARIOS {
        let picks: Vec<QoSProfile> = arms
            .iter()
            .map(|(_, d)| d.decide(&qm(vals)).profile)
            .collect();
        let first = picks[0].clone();
        if picks.iter().any(|p| *p != first) {
            let picks_str: Vec<String> = arms
                .iter()
                .zip(picks.iter())
                .map(|((n, _), p)| format!("{n}={}", short(p)))
                .collect();
            println!("{name:<12}: {}", picks_str.join(", "));
        }
    }

    println!("\n(notas: latências são ns/decide locais neste processo; o fcm-dhl aprende");
    println!(" online entre cenários — a sequência influencia suas decisões.)");
    println!("(métricas de referência: urgência, deadline, latência, carga, erro, confiança,");
    println!(" complexidade, streaming — nesta ordem: {:?})", KEYS);
}
