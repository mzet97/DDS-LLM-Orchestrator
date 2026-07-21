//! Testes do llm-gateway: worker pool paralelo (T-420), roteamento (T-421),
//! cache + rate-limit + 429 (T-422), failover (T-424).

use dds_contract::generated::orchestrator::LLMInferenceRequest;
use llm_gateway::{
    CircuitBreaker, FailoverTarget, GatewayError, GatewayProviders, LlmGateway, MockProvider,
    Provider,
};
use std::sync::atomic::{AtomicU64, AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant};

fn req(id: &str, constraint: &str) -> LLMInferenceRequest {
    LLMInferenceRequest {
        request_id: id.into(),
        task_id: id.into(),
        agent_id: "tester".into(),
        model_name: "qwen".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 16,
        stream: false,
        security_level: 0,
        provider_constraint: constraint.into(),
        created_at_ns: 0,
    }
}

fn mock(kind: Provider, delay_ms: u64) -> (Arc<MockProvider>, Arc<AtomicUsize>, Arc<AtomicU64>) {
    let max_concurrent = Arc::new(AtomicUsize::new(0));
    let calls = Arc::new(AtomicU64::new(0));
    let p = Arc::new(MockProvider {
        kind,
        delay: Duration::from_millis(delay_ms),
        concurrent: Arc::new(AtomicUsize::new(0)),
        max_concurrent: max_concurrent.clone(),
        calls: calls.clone(),
        fail: false,
    });
    (p, max_concurrent, calls)
}

/// Mock provider que sempre falha (para testar failover).
fn failing_mock(kind: Provider) -> Arc<MockProvider> {
    Arc::new(MockProvider {
        kind,
        delay: Duration::from_millis(1),
        concurrent: Arc::new(AtomicUsize::new(0)),
        max_concurrent: Arc::new(AtomicUsize::new(0)),
        calls: Arc::new(AtomicU64::new(0)),
        fail: true,
    })
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t420_worker_pool_paralelo_com_metricas() {
    let (local, max_conc, _) = mock(Provider::Local, 100);
    let providers = GatewayProviders::new(Some(local), None);
    let gw = Arc::new(LlmGateway::new(2, 100, 16));

    let t0 = Instant::now();
    let mut set = tokio::task::JoinSet::new();
    for i in 0..4 {
        let gw = Arc::clone(&gw);
        let providers = Arc::new(providers.clone());
        set.spawn(async move {
            gw.process_routed(&providers, req(&format!("r{i}"), "ANY"))
                .await
        });
    }
    let mut oks = 0;
    while let Some(r) = set.join_next().await {
        assert!(r.unwrap().is_ok());
        oks += 1;
    }
    let elapsed = t0.elapsed();

    assert_eq!(oks, 4);
    let mc = max_conc.load(Ordering::Relaxed);
    assert!(mc >= 2, "sem paralelismo: max={mc}");
    assert!(mc <= 2, "excedeu workers=2: max={mc}");
    // 4 reqs de 100ms em 2 workers ≈ 200ms (2 ondas), nunca ~400ms serial
    assert!(
        elapsed < Duration::from_millis(350),
        "demorou {elapsed:?} — pool não paraleliza"
    );

    let m = gw.metrics();
    assert_eq!(m.total_requests.load(Ordering::Relaxed), 4);
    assert_eq!(m.errors.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t421_roteamento_por_constraint() {
    let (local, _, calls_local) = mock(Provider::Local, 10);
    let (cloud, _, calls_cloud) = mock(Provider::Cloud, 10);
    let providers = GatewayProviders::new(Some(local), Some(cloud));
    let gw = LlmGateway::new(4, 100, 0);

    gw.process_routed(&providers, req("a", "LOCAL_ONLY"))
        .await
        .unwrap();
    assert_eq!(calls_local.load(Ordering::Relaxed), 1);
    assert_eq!(calls_cloud.load(Ordering::Relaxed), 0);

    gw.process_routed(&providers, req("b", "CLOUD_ONLY"))
        .await
        .unwrap();
    assert_eq!(calls_cloud.load(Ordering::Relaxed), 1);

    // Constraint "ANY" → prefere local
    gw.process_routed(&providers, req("c", "ANY"))
        .await
        .unwrap();
    assert_eq!(calls_local.load(Ordering::Relaxed), 2);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t422_cache_rate_limit_e_429() {
    let (local, _, calls) = mock(Provider::Local, 20);
    let providers = GatewayProviders::new(Some(local), None);
    // rate_limit=1 token (refil 1/s)
    let gw = LlmGateway::new(2, 1, 16);

    // 1ª chamada: passa no rate limit e vai ao provider
    gw.process_routed(&providers, req("x", "ANY"))
        .await
        .unwrap();
    assert_eq!(calls.load(Ordering::Relaxed), 1);

    // 2ª chamada MESMO CONTEÚDO: cache hit (não chama provider, não estoura rate)
    gw.process_routed(&providers, req("x", "ANY"))
        .await
        .unwrap();
    assert_eq!(
        calls.load(Ordering::Relaxed),
        1,
        "cache não segurou a 2ª chamada"
    );
    assert_eq!(gw.metrics().cache_hits.load(Ordering::Relaxed), 1);

    // 3ª chamada CONTEÚDO DIFERENTE: estoura o rate limit → 429 retriable
    let mut req_y = req("y", "ANY");
    req_y.messages_json = r#"[{"role":"user","content":"y"}]"#.into();
    let err = gw.process_routed(&providers, req_y).await.unwrap_err();
    match &err {
        GatewayError::RateLimited(_) => {}
        other => panic!("esperava RateLimited, veio {other:?}"),
    }
    let llm_err = err.to_llm_error("y");
    assert_eq!(llm_err.error_code, 429);
    assert!(llm_err.retriable);
    assert_eq!(llm_err.request_id, "y");
    assert_eq!(gw.metrics().rate_limited.load(Ordering::Relaxed), 1);
}

// ── Failover tests (T-424) ──────────────────────────────────────────────────

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t424_failover_sucesso() {
    // Primary: always fails. Failover target: always succeeds.
    let primary = failing_mock(Provider::Local);
    let (failover, _, failover_calls) = mock(Provider::Cloud, 10);

    let mut providers = GatewayProviders::new(Some(primary), None);
    let cb = Arc::new(CircuitBreaker::new(3, Duration::from_secs(60)));
    providers.register_failover(
        "local",
        vec![FailoverTarget {
            provider: failover,
            model: "qwen3-4b".into(),
            circuit_breaker: cb,
            priority: 1,
        }],
    );

    let gw = LlmGateway::new(4, 100, 0);
    let result = gw
        .process_routed(&providers, req("fo-1", "LOCAL_ONLY"))
        .await;
    assert!(
        result.is_ok(),
        "failover deveria ter sucesso: {:?}",
        result.err()
    );
    assert_eq!(failover_calls.load(Ordering::Relaxed), 1);
    assert_eq!(gw.metrics().failover_successes.load(Ordering::Relaxed), 1);
    assert_eq!(gw.metrics().failover_failures.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t424_failover_todos_falham() {
    // Both primary and failover fail.
    let primary = failing_mock(Provider::Local);
    let failover = failing_mock(Provider::Cloud);

    let mut providers = GatewayProviders::new(Some(primary), None);
    let cb = Arc::new(CircuitBreaker::new(3, Duration::from_secs(60)));
    providers.register_failover(
        "local",
        vec![FailoverTarget {
            provider: failover,
            model: "qwen3-4b".into(),
            circuit_breaker: cb,
            priority: 1,
        }],
    );

    let gw = LlmGateway::new(4, 100, 0);
    let result = gw
        .process_routed(&providers, req("fo-2", "LOCAL_ONLY"))
        .await;
    assert!(
        result.is_err(),
        "deveria falhar quando todos os targets falham"
    );
    assert_eq!(gw.metrics().failover_failures.load(Ordering::Relaxed), 1);
    assert_eq!(gw.metrics().failover_successes.load(Ordering::Relaxed), 0);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t424_failover_circuit_breaker_aberto() {
    // Primary fails. Failover target has circuit breaker open (too many failures).
    let primary = failing_mock(Provider::Local);
    let (failover, _, failover_calls) = mock(Provider::Cloud, 10);

    let mut providers = GatewayProviders::new(Some(primary), None);
    let cb = Arc::new(CircuitBreaker::new(1, Duration::from_secs(60)));
    // Trigger circuit breaker open
    cb.record_failure();
    assert!(!cb.is_available(), "circuit breaker deveria estar aberto");

    providers.register_failover(
        "local",
        vec![FailoverTarget {
            provider: failover,
            model: "qwen3-4b".into(),
            circuit_breaker: cb,
            priority: 1,
        }],
    );

    let gw = LlmGateway::new(4, 100, 0);
    let result = gw
        .process_routed(&providers, req("fo-3", "LOCAL_ONLY"))
        .await;
    assert!(
        result.is_err(),
        "deveria falhar quando circuit breaker está aberto"
    );
    assert_eq!(
        failover_calls.load(Ordering::Relaxed),
        0,
        "não deveria tentar failover com CB aberto"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn t424_failover_multiplos_targets_prioridade() {
    // Primary fails. Two failover targets: first has CB open, second succeeds.
    let primary = failing_mock(Provider::Local);
    let failover1 = failing_mock(Provider::Cloud);
    let (failover2, _, calls2) = mock(Provider::Cloud, 10);

    let mut providers = GatewayProviders::new(Some(primary), None);

    let cb1 = Arc::new(CircuitBreaker::new(1, Duration::from_secs(60)));
    cb1.record_failure(); // open

    let cb2 = Arc::new(CircuitBreaker::new(3, Duration::from_secs(60)));

    providers.register_failover(
        "local",
        vec![
            FailoverTarget {
                provider: failover1,
                model: "gpt-4o".into(),
                circuit_breaker: cb1,
                priority: 1,
            },
            FailoverTarget {
                provider: failover2,
                model: "qwen3-4b".into(),
                circuit_breaker: cb2,
                priority: 2,
            },
        ],
    );

    let gw = LlmGateway::new(4, 100, 0);
    let result = gw
        .process_routed(&providers, req("fo-4", "LOCAL_ONLY"))
        .await;
    assert!(
        result.is_ok(),
        "deveria usar o segundo failover target: {:?}",
        result.err()
    );
    assert_eq!(calls2.load(Ordering::Relaxed), 1);
    assert_eq!(gw.metrics().failover_successes.load(Ordering::Relaxed), 1);
}
