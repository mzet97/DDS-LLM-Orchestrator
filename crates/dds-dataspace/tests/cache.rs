//! Testes dos caches concorrentes (T-303).
//!
//! Aceite: teste concorrente sem corrupção; disputa sem regressão.

use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_dataspace::cache::TopicCaches;
use std::sync::Arc;

fn make_task(id: &str, status: i32, ts: u64) -> Task {
    Task {
        task_id: id.into(),
        client_id: "c".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 8,
        stream: false,
        status,
        priority: 5,
        created_at_ns: ts,
        assigned_at_ns: if status >= 1 { ts + 1 } else { 0 },
        started_at_ns: if status >= 2 { ts + 2 } else { 0 },
        completed_at_ns: if status >= 3 { ts + 3 } else { 0 },
        deadline_ns: ts + 60_000_000_000,
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

#[test]
fn upsert_bloqueia_regressao_de_status() {
    let caches = TopicCaches::new();

    // status avança: aceito
    let t1 = caches.upsert_task(make_task("t1", 0, 100));
    assert_eq!(t1.status, 0);
    let t2 = caches.upsert_task(make_task("t1", 2, 200));
    assert_eq!(t2.status, 2);

    // regressão: rejeitado (fica o status 2)
    let t3 = caches.upsert_task(make_task("t1", 1, 300));
    assert_eq!(t3.status, 2, "regressão de status passou!");

    // mesmo status: o incoming vence (last-write-wins por chegada — reflete a
    // arbitragem do mesh; timestamps NÃO decidem)
    let t4 = caches.upsert_task(make_task("t1", 2, 400));
    assert_eq!(t4.assigned_at_ns, 401);
    let t5 = caches.upsert_task(make_task("t1", 2, 150));
    assert_eq!(
        t5.assigned_at_ns, 151,
        "last-write-wins: o incoming deve vencer"
    );
}

#[test]
fn outputs_dedup_por_seq_num() {
    let caches = TopicCaches::new();
    let out = |seq: u32, ts: u64| dds_contract::generated::dds_llm_orchestrator::TaskOutput {
        task_id: "t1".into(),
        seq_num: seq,
        content: format!("c{seq}"),
        is_final: false,
        finish_reason: 0,
        agent_id: "a".into(),
        token_count: 1,
        emitted_at_ns: ts,
    };
    caches.push_output(out(0, 10));
    caches.push_output(out(1, 20));
    caches.push_output(out(0, 30)); // reentrega com ts maior → substitui
    caches.push_output(out(0, 15)); // reentrega antiga → ignora

    let outs = caches.outputs_of("t1");
    assert_eq!(outs.len(), 2, "reentrega duplicou seq_num");
    assert_eq!(outs[0].emitted_at_ns, 30);
}

#[tokio::test(flavor = "multi_thread", worker_threads = 16)]
async fn stress_concorrencia_sem_corrupcao() {
    let caches = Arc::new(TopicCaches::new());
    const WRITERS: usize = 8;
    const PER_WRITER: usize = 200;

    let mut handles = Vec::new();
    for w in 0..WRITERS {
        let c = Arc::clone(&caches);
        handles.push(tokio::spawn(async move {
            for i in 0..PER_WRITER {
                let id = format!("task-{w}-{i}");
                c.upsert_task(make_task(&id, 0, 100));
                // avança status (monotônico)
                c.upsert_task(make_task(&id, 3, 200));
            }
        }));
    }
    // Leitores concorrentes
    for _ in 0..4 {
        let c = Arc::clone(&caches);
        handles.push(tokio::spawn(async move {
            for _ in 0..500 {
                let all = c.all_tasks();
                for t in all {
                    // leitura consistente: status nunca regride numa mesma leitura
                    assert!(
                        t.status == 0 || t.status == 3,
                        "status intermediário corrompido: {}",
                        t.status
                    );
                }
                tokio::task::yield_now().await;
            }
        }));
    }
    for h in handles {
        h.await.unwrap();
    }

    let all = caches.all_tasks();
    assert_eq!(all.len(), WRITERS * PER_WRITER);
    assert!(
        all.iter().all(|t| t.status == 3),
        "nem toda task chegou ao status 3"
    );
}

#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn disputa_mesmo_id_sem_regressao() {
    for round in 0..200 {
        let caches = Arc::new(TopicCaches::new());
        let c1 = Arc::clone(&caches);
        let c2 = Arc::clone(&caches);
        let id = format!("race-{round}");
        let id2 = id.clone();
        let h1 = tokio::spawn(async move { c1.upsert_task(make_task(&id, 1, 100)) });
        let h2 = tokio::spawn(async move { c2.upsert_task(make_task(&id2, 2, 50)) });
        let _ = tokio::join!(h1, h2);
        let final_ = caches.read_task(&format!("race-{round}")).unwrap();
        assert_eq!(final_.status, 2, "versão mais fraca venceu a disputa");
    }
}
