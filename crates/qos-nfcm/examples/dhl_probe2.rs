use qos_nfcm::decider::{QoSMetrics, QosDecider};
use qos_nfcm::fcm::FcmDhlDecider;

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

fn main() {
    let dhl = FcmDhlDecider::new(0.1);
    let hi = [0.70, 0.30, 0.50, 0.30, 0.10, 0.80, 0.90, 0.10];
    let lo = [0.70, 0.30, 0.50, 0.30, 0.10, 0.80, 0.10, 0.10];
    for _ in 0..8 {
        let _ = dhl.decide(&qm(&lo));
        let _ = dhl.decide(&qm(&hi));
    }
    println!(
        "peso estimated_complexity→QoS_Critical: {:.4} (inicial 0.050)",
        dhl.weight_of("estimated_complexity", "QoS_Critical")
            .unwrap()
    );
    println!(
        "peso error_rate→QoS_Failover: {:.4} (inicial 0.250)",
        dhl.weight_of("error_rate", "QoS_Failover").unwrap()
    );
}
