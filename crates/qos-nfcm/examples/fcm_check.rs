use qos_nfcm::decider::{QoSMetrics, QosDecider};
use qos_nfcm::fcm::FcmDecider;
fn main() {
    let fcm = FcmDecider::new();
    for (nome, m) in [
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
    ] {
        let q = QoSMetrics {
            urgency: m[0],
            deadline_pressure: m[1],
            recent_latency: m[2],
            agent_load: m[3],
            error_rate: m[4],
            historical_confidence: m[5],
            estimated_complexity: m[6],
            streaming_need: m[7],
        };
        let d = fcm.decide(&q);
        println!("{nome}: {:?} (conf {:.3})", d.profile, d.confidence);
    }
}
