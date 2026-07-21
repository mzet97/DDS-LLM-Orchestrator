//! Teste de aceite T-201: trait Engine + MockEngine (chunks previsíveis).

use agent::engine::{Chunk, Engine, InferRequest, MockEngine};
use futures_util::StreamExt;

fn req() -> InferRequest {
    InferRequest {
        request_id: "req-1".into(),
        messages_json: "[]".into(),
        model_name: "qwen".into(),
        temperature: 0.7,
        max_tokens: 32,
        stream: true,
        timeout_ms: 1000,
    }
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_engine_emite_chunks_previsiveis() {
    let engine = MockEngine::new("tok", 5, 0);
    let mut stream = engine.infer_stream(req());

    let mut chunks: Vec<Chunk> = Vec::new();
    while let Some(c) = stream.next().await {
        chunks.push(c.expect("chunk ok"));
    }

    assert_eq!(chunks.len(), 5);
    for (i, c) in chunks.iter().enumerate() {
        assert_eq!(c.seq_num, i as u32);
        assert_eq!(c.content, format!("tok-{i:04}"));
        assert_eq!(c.is_final, i == 4);
        assert_eq!(c.tokens_prompt, 10);
    }
    assert_eq!(chunks[4].tokens_completion, 5);
    assert!(chunks[..4].iter().all(|c| c.tokens_completion == 1));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn mock_engine_respeita_delay() {
    let engine = MockEngine::new("x", 3, 50);
    let t0 = std::time::Instant::now();
    let mut stream = engine.infer_stream(req());
    let mut n = 0;
    while let Some(c) = stream.next().await {
        c.unwrap();
        n += 1;
    }
    assert_eq!(n, 3);
    assert!(t0.elapsed() >= std::time::Duration::from_millis(150));
}

#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn engine_trait_e_objeto_seguro_para_threads() {
    fn assert_engine<E: Engine>(_: &E) {}
    let engine = MockEngine::new("x", 1, 0);
    assert_engine(&engine);
}
