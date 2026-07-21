use qos_nfcm::decider::{QoSMetrics, QosDecider};
use qos_nfcm::fcm::{FcmDecider, FcmDhlDecider};

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
    // Série: error_rate oscilando alto/baixo (o FCM produz Failover alto/baixo junto)
    let dhl = FcmDhlDecider::new(0.1);
    let hi = [0.30, 0.30, 0.60, 0.30, 0.90, 0.40, 0.30, 0.10];
    let lo = [0.30, 0.30, 0.40, 0.30, 0.20, 0.40, 0.30, 0.10];
    for _ in 0..6 {
        let _ = dhl.decide(&qm(&lo));
        let _ = dhl.decide(&qm(&hi));
    }
    println!(
        "peso error_rate→QoS_Failover: {:.3} (inicial 0.250)",
        dhl.weight_of("error_rate", "QoS_Failover").unwrap()
    );

    // Input fronteiriço: error moderado-baixo
    let x = [0.30, 0.30, 0.50, 0.30, 0.35, 0.50, 0.30, 0.10];
    let plain = FcmDecider::new().decide(&qm(&x));
    println!("plain FCM: {:?} ({:.3})", plain.profile, plain.confidence);
    let learned = dhl.decide(&qm(&x));
    println!(
        "DHL após aprender: {:?} ({:.3})",
        learned.profile, learned.confidence
    );
}
