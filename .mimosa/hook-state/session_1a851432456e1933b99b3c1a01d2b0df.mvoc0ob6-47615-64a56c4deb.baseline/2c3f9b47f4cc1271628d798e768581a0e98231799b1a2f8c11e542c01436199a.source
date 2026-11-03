//! Exporta em CSV a trajetória de convergência de `h` no cenário "degradado"
//! do artigo (Seção 6). Usado para regenerar `fig_nfcm_convergencia.png`.
//!
//! Uso: cargo run -p qos-nfcm --example degradado_convergencia > convergencia.csv

use qos_nfcm::{Nfcm, NODES};

fn main() {
    let x = [0.60, 0.40, 0.85, 0.80, 0.90, 0.20, 0.50, 0.10];
    let r = Nfcm::qos_default().infer(&x);

    print!("iteracao");
    for name in NODES {
        print!(",{name}");
    }
    println!();

    for (it, h) in r.h_history.iter().enumerate() {
        print!("{it}");
        for v in h {
            print!(",{v:.6}");
        }
        println!();
    }
}
