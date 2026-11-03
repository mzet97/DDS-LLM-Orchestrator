//! Demonstração ao vivo: roda a inferência NFCM real (não é teste) sobre os
//! quatro cenários canônicos e imprime a decisão e a explicação causal
//! produzidas pelo modelo. Uso: cargo run -p qos-nfcm --example demo_decisoes

use qos_nfcm::{explain_text, Nfcm, METRICS};

fn main() {
    let nfcm = Nfcm::qos_default();
    let cenarios: [(&str, [f64; 8]); 4] = [
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
    ];

    for (nome, x) in cenarios {
        let r = nfcm.infer(&x);
        println!("=== cenário: {nome} ===");
        for (m, v) in METRICS.iter().zip(x.iter()) {
            print!("{m}={v:.2} ");
        }
        println!();
        println!(
            "  h_final = pressure={:.3} health={:.3} stream={:.3}  (convergiu={}, {} iterações)",
            r.h_final[0], r.h_final[1], r.h_final[2], r.converged, r.iterations
        );
        println!("  {}", explain_text(&r));
        println!();
    }
}
