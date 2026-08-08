//! Testes T-305: pool de writers MPMC (throughput real DDS) + backpressure.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::{Task, TaskOutput};
use dds_dataspace::api::DataSpaceError;
use dds_dataspace::writer_pool::{WriteRequest, WriterPool};
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::sync::atomic::{AtomicUsize, Ordering};
use std::sync::Arc;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 84;

fn make_task(id: &str) -> Task {
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    Task {
        task_id: id.into(),
        client_id: "pool".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen3.5-0.8b".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 16,
        stream: false,
        status: 0,
        priority: 5,
        created_at_ns: now_ns,
        assigned_at_ns: 0,
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns: now_ns + 60_000_000_000,
        retry_count: 0,
        finish_reason: String::new(),
        t_serialization_ns: 0,
        t_transport_send_ns: 0,
        t_agent_queue_ns: 0,
        t_inference_ns: 0,
        t_transport_return_ns: 0,
        t_deserialization_ns: 0,
    }
}

/// Throughput: 5.000 tasks por 8 threads num pool K=4 — nenhuma perda SILENCIOSA.
///
/// Contrato pós-RUST-CACHE-006: o cache do assinante guarda no máximo 2.048
/// tasks não-terminais; o excedente é REJEITADO explicitamente (contador
/// `tasks_rejected`) e não é entregue ao stream. O teste publica 5.000 ids
/// distintos (todos PENDING, logo nada evictável) e exige que cada amostra
/// seja ou entregue (e legível via read_task) ou contabilizada como rejeitada.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn writer_pool_throughput_5k() {
    let ds_pub = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let ds_sub = DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let pool = ds_pub.new_writer_pool(4, 8_192);

    const N: usize = 5000;
    const CACHE_CAP: usize = 2048; // MAX_TASKS_IN_CACHE (cache.rs)
    let counter = Arc::new(AtomicUsize::new(0));

    // Assinante conta as ENTREGUES (Accepted) via stream; rejeitadas saem
    // pelo contador do cache. Para quando delivered + rejected == N.
    let counter2 = Arc::clone(&counter);
    let caches_sub = ds_sub.caches();
    let caches_probe = Arc::clone(&caches_sub);
    let delivered_ids = Arc::new(std::sync::Mutex::new(Vec::<String>::new()));
    let delivered_ids2 = Arc::clone(&delivered_ids);
    let mut stream = Box::pin(ds_sub.stream_tasks());
    let collector = tokio::spawn(async move {
        loop {
            let delivered = counter2.load(Ordering::Relaxed);
            let rejected = caches_probe.task_cache_stats().tasks_rejected as usize;
            if delivered + rejected >= N {
                break;
            }
            match tokio::time::timeout(Duration::from_secs(30), stream.next()).await {
                Ok(Some(task)) => {
                    // guarda os primeiros ids ENTREGUES para o probe de readback
                    let mut ids = delivered_ids2.lock().unwrap();
                    if ids.len() < 3 {
                        ids.push(task.task_id.clone());
                    }
                    drop(ids);
                    counter2.fetch_add(1, Ordering::Relaxed);
                }
                Ok(None) => break,   // stream encerrado
                Err(_) => {}         // timeout parcial: re-checa contadores
            }
        }
        counter2.load(Ordering::Relaxed)
    });

    tokio::time::sleep(Duration::from_millis(2000)).await; // settle/match

    let t0 = Instant::now();
    let mut handles = Vec::new();
    let pool = Arc::new(pool);
    for th in 0..8 {
        let p = Arc::clone(&pool);
        handles.push(std::thread::spawn(move || {
            for i in 0..(N / 8) {
                p.submit(WriteRequest::Task(make_task(&format!("pool-{th}-{i}"))))
                    .expect("submit");
            }
        }));
    }
    for h in handles {
        h.join().unwrap();
    }

    // Timeout global: sem ele, qualquer amostra perdida fazia o teste rodar
    // para sempre (o collector descarta cada timeout de 30 s e re-tenta sem
    // limite) — observado na validação da Fase 2 (>110 min pendurado).
    let got = tokio::time::timeout(Duration::from_secs(180), collector)
        .await
        .unwrap_or_else(|_| {
            panic!(
                "collector não terminou em 180 s: {}/{} amostras recebidas",
                counter.load(Ordering::Relaxed),
                N
            )
        })
        .unwrap();
    let elapsed = t0.elapsed();
    let stats = caches_sub.task_cache_stats();
    let rejected = stats.tasks_rejected as usize;
    println!(
        "[T-305] {} entregues + {} rejeitadas = {} publicadas em {:?} ({:.0} tasks/s aceitas)",
        got,
        rejected,
        got + rejected,
        elapsed,
        got as f64 / elapsed.as_secs_f64()
    );

    // (1) Nenhuma perda silenciosa: toda amostra foi entregue ou rejeitada.
    assert_eq!(
        got + rejected,
        N,
        "perda silenciosa: entregues+rejeitadas != publicadas"
    );
    // (2) O cap engatou: só as primeiras ~CACHE_CAP foram aceitas/entregues.
    assert!(
        got <= CACHE_CAP && rejected > 0,
        "cap de {CACHE_CAP} não engatou: {got} entregues, {rejected} rejeitadas"
    );
    // (3) Invariante stream→readback: amostra ENTREGUE é legível do cache
    // (sonda ids que o stream de fato entregou — a ordem de entrega DDS não é
    // a ordem de publicação, então não dá para adivinhar ids aceitos).
    for id in delivered_ids.lock().unwrap().iter() {
        assert!(
            caches_sub.read_task(id).is_some(),
            "task entregue {id} não está no cache"
        );
    }
    assert_eq!(pool.failed(), 0, "backpressure não deveria disparar a 5k");

    let pool = Arc::try_unwrap(pool).ok().expect("pool sem refs");
    pool.drain_and_shutdown();
    ds_pub.shutdown().await.unwrap();
    ds_sub.shutdown().await.unwrap();
}

/// Backpressure: fila minúscula + consumidor lento → submit falha rápido.
#[tokio::test(flavor = "multi_thread", worker_threads = 2)]
async fn writer_pool_backpressure_fail_fast() {
    // write_fn lento (mock — sem DDS): 50 ms por item
    let slow = Arc::new(|_req: WriteRequest| {
        std::thread::sleep(Duration::from_millis(50));
    });
    let pool = WriterPool::new(1, 4, slow);

    let mut failed = 0;
    for i in 0..16 {
        if pool
            .submit(WriteRequest::Task(make_task(&format!("bp-{i}"))))
            .is_err()
        {
            failed += 1;
        }
    }
    println!("[T-305] backpressure: {failed}/16 submits rejeitados com fila cheia");
    assert!(
        failed > 0,
        "backpressure não disparou com fila de 4 e consumidor lento"
    );
    assert_eq!(pool.failed() as usize, failed);
    pool.drain_and_shutdown();
}

fn make_final_output(task_id: &str) -> TaskOutput {
    TaskOutput {
        task_id: task_id.into(),
        seq_num: 7,
        content: "fim".into(),
        is_final: true,
        finish_reason: 1,
        agent_id: "agent-test".into(),
        token_count: 42,
        emitted_at_ns: 123,
    }
}

/// RUST-PROTO-005: o ack do write final carrega o resultado REAL do write —
/// sucesso confirmado chega como Ok ao produtor.
#[tokio::test]
async fn ack_write_final_confirma_sucesso() {
    let write_fn = Arc::new(|req: WriteRequest| {
        if let WriteRequest::OutputAck(_o, ack) = req {
            let _ = ack.send(Ok(()));
        }
    });
    let pool = WriterPool::new(1, 8, write_fn);

    let rx = pool
        .submit_with_ack(make_final_output("ack-ok"))
        .expect("submit_with_ack");
    let result = tokio::time::timeout(Duration::from_secs(5), rx)
        .await
        .expect("ack dentro do prazo")
        .expect("canal de ack aberto");
    assert!(result.is_ok(), "write final bem-sucedido deve confirmar Ok");

    pool.drain_and_shutdown();
}

/// RUST-PROTO-005: falha do write APÓS o enqueue chega ao produtor como Err —
/// é o que permite o agente publicar FAILED em vez de DONE sem saída final.
#[tokio::test]
async fn ack_write_final_propaga_falha_pos_enqueue() {
    let write_fn = Arc::new(|req: WriteRequest| {
        if let WriteRequest::OutputAck(_o, ack) = req {
            let _ = ack.send(Err(DataSpaceError::WriteFailed("falha injetada".into())));
        }
    });
    let pool = WriterPool::new(1, 8, write_fn);

    // enqueue tem sucesso; o fracasso aparece somente no ack.
    let rx = pool
        .submit_with_ack(make_final_output("ack-fail"))
        .expect("enqueue ok");
    let result = tokio::time::timeout(Duration::from_secs(5), rx)
        .await
        .expect("ack dentro do prazo")
        .expect("canal de ack aberto");
    let err = result.expect_err("write final falho deve chegar como Err");
    assert!(err.to_string().contains("falha injetada"));

    pool.drain_and_shutdown();
}

/// RUST-PROTO-005: shutdown drena a fila e responde acks pendentes — nenhum
/// produtor fica esperando confirmação que nunca chega.
#[tokio::test]
async fn drain_responde_acks_pendentes() {
    let write_fn = Arc::new(|req: WriteRequest| {
        if let WriteRequest::OutputAck(_o, ack) = req {
            std::thread::sleep(Duration::from_millis(20));
            let _ = ack.send(Ok(()));
        }
    });
    let pool = WriterPool::new(1, 8, write_fn);

    let rx1 = pool.submit_with_ack(make_final_output("drain-1")).unwrap();
    let rx2 = pool.submit_with_ack(make_final_output("drain-2")).unwrap();
    pool.drain_and_shutdown();

    for rx in [rx1, rx2] {
        let result = tokio::time::timeout(Duration::from_secs(5), rx)
            .await
            .expect("ack respondido no drain")
            .expect("canal aberto");
        assert!(result.is_ok());
    }
}
