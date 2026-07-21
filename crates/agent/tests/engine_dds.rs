//! Teste do DdsEngine contra o llama-server real (T-205).
//!
//! Requer o llama-server rodando com `--enable-dds --dds-domain 91`:
//!   /home/mzet/.cache/llama-build/bin/llama-server -m <modelo.gguf> \
//!       --enable-dds --dds-domain 91 --port 8091
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p agent --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use agent::engine::{Engine, InferRequest};
use agent::engine_dds::DdsEngine;
use futures_util::StreamExt;
use std::time::Duration;

const DOMAIN: u32 = 91;

async fn llama_server_up() -> bool {
    for _ in 0..60 {
        if tokio::process::Command::new("curl")
            .args(["-sf", "http://127.0.0.1:8091/health"])
            .output()
            .await
            .map(|o| o.status.success())
            .unwrap_or(false)
        {
            return true;
        }
        tokio::time::sleep(Duration::from_millis(500)).await;
    }
    false
}

// Ignorado por padrão: requer um llama-server externo real com `--enable-dds`
// no domínio 91 (ver cabeçalho). Sob `cargo test --workspace` a unificação de
// features do Cargo liga `dds` e este teste rodaria junto — mas sem o servidor
// (ou com algo respondendo `/health` sem o lado DDS) ele bloqueia até o timeout
// de 60 s e falha o gate. Rode-o deliberadamente:
//   CYCLONEDDS_STATIC=1 cargo test -p agent --features dds -- --ignored --test-threads=1
#[ignore = "requer llama-server real (--enable-dds) em 127.0.0.1:8091 no domínio 91"]
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn dds_engine_infer_real() {
    if !llama_server_up().await {
        eprintln!("[T-205] llama-server indisponível no domínio {DOMAIN} — pulando");
        return;
    }

    let engine = DdsEngine::new(DOMAIN, "agent-test-t205".into()).unwrap();
    let req = InferRequest {
        request_id: "t205-req-1".into(),
        messages_json: r#"[{"role":"user","content":"Reply with exactly the word: OK"}]"#.into(),
        model_name: "qwen".into(),
        temperature: 0.0,
        max_tokens: 16,
        stream: true,
        timeout_ms: 60_000,
    };

    let mut chunks = Vec::new();
    let mut stream = engine.infer_stream(req);
    while let Some(c) = stream.next().await {
        chunks.push(c.expect("chunk ok"));
    }

    assert!(!chunks.is_empty(), "nenhum chunk recebido do llama-server");
    assert!(chunks.last().unwrap().is_final, "último chunk não é final");
    let text: String = chunks.iter().map(|c| c.content.clone()).collect();
    // A resposta pode ser vazia ocasionalmente (decisão do modelo); o que o
    // teste garante é o round-trip do protocolo DDS (request→result correlacionados).
    println!(
        "[T-205] resposta do llama-server: {text:?} ({} chunks)",
        chunks.len()
    );
}
