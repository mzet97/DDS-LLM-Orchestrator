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

#[test]
fn outputs_limitam_chunks_e_chaves_distintas() {
    let caches = TopicCaches::new();
    let output =
        |task_id: String, seq_num: u32| dds_contract::generated::dds_llm_orchestrator::TaskOutput {
            task_id,
            seq_num,
            content: String::new(),
            is_final: false,
            finish_reason: 0,
            agent_id: "a".into(),
            token_count: 0,
            emitted_at_ns: seq_num as u64,
        };

    for seq_num in 0..300 {
        caches.push_output(output("uma-task".into(), seq_num));
    }
    assert_eq!(caches.outputs_of("uma-task").len(), 256);

    for task_num in 0..2050 {
        caches.push_output(output(format!("task-{task_num}"), 0));
    }
    assert!(caches.outputs.len() <= 2048);
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
    // Regressão de RUST-CACHE-006B: duas primeiras-inserções concorrentes do
    // mesmo task_id, em ordens inversas de status/timestamp por rodada.
    // 10.000 disputas (aceite da Fase 2.1 do plano de correção).
    for round in 0..10_000 {
        let caches = Arc::new(TopicCaches::new());
        let c1 = Arc::clone(&caches);
        let c2 = Arc::clone(&caches);
        let id = format!("race-{round}");
        let id2 = id.clone();
        // Alterna quem dispara primeiro entre a versão fraca e a forte.
        let (h1, h2) = if round % 2 == 0 {
            (
                tokio::spawn(async move { c1.upsert_task(make_task(&id, 1, 100)) }),
                tokio::spawn(async move { c2.upsert_task(make_task(&id2, 2, 50)) }),
            )
        } else {
            (
                tokio::spawn(async move { c1.upsert_task(make_task(&id, 2, 50)) }),
                tokio::spawn(async move { c2.upsert_task(make_task(&id2, 1, 100)) }),
            )
        };
        let _ = tokio::join!(h1, h2);
        let final_ = caches.read_task(&format!("race-{round}")).unwrap();
        assert_eq!(final_.status, 2, "versão mais fraca venceu a disputa");
    }
}

#[test]
fn primeira_insercao_deterministica_ambas_as_ordens() {
    // Versão sequencial (determinística) da disputa: cobre as duas ordens
    // sem depender de scheduling.
    for (s1, s2) in [(1, 2), (2, 1)] {
        let caches = TopicCaches::new();
        caches.upsert_task(make_task("det", s1, 100));
        let out = caches.upsert_task(make_task("det", s2, 50));
        assert!(out.is_accepted());
        assert_eq!(
            caches.read_task("det").unwrap().status,
            2,
            "ordem ({s1} -> {s2}) regrediu o status"
        );
    }
}

#[test]
fn cache_saturado_rejeita_sem_entregar_e_recupera_apos_eviction() {
    // RUST-CACHE-006: toda amostra Accepted é imediatamente legível; após o
    // cap, eviction de terminais volta a aceitar tasks novas.
    let caches = TopicCaches::new();
    for i in 0..2048 {
        let r = caches.upsert_task(make_task(&format!("fill-{i}"), 0, 100 + i as u64));
        assert!(r.is_accepted(), "inserção {i} deveria ser aceita");
    }

    // Saturado: task nova é rejeitada e NÃO pode ser lida de volta.
    let rejected = caches.upsert_task(make_task("overflow-1", 0, 9999));
    assert!(
        !rejected.is_accepted(),
        "task nova deveria ser rejeitada no cap"
    );
    assert!(
        caches.read_task("overflow-1").is_none(),
        "amostra rejeitada não pode estar no cache"
    );
    let stats = caches.task_cache_stats();
    assert_eq!(stats.tasks_rejected, 1);
    assert_eq!(stats.tasks_len, 2048);

    // Tasks existentes continuam atualizáveis mesmo no cap (Occupied path).
    assert!(caches
        .upsert_task(make_task("fill-0", 1, 200))
        .is_accepted());

    // Marca uma task como terminal antiga → o próximo upsert de id novo
    // dispara eviction sob pressão e passa a ser aceito.
    let mut terminal = make_task("fill-1", 3, 50);
    terminal.completed_at_ns = 53; // ns epoch → muito mais velho que o TTL
    assert!(caches.upsert_task(terminal).is_accepted());

    let accepted = caches.upsert_task(make_task("overflow-2", 0, 8888));
    assert!(
        accepted.is_accepted(),
        "após eviction de terminais o cache deve aceitar novas tasks"
    );
    assert!(
        caches.read_task("overflow-2").is_some(),
        "toda amostra Accepted deve ser legível via read_task"
    );
    assert!(
        caches.read_task("fill-1").is_none(),
        "terminal antiga deveria ter sido evictada sob pressão"
    );
    let stats = caches.task_cache_stats();
    assert!(stats.tasks_evicted >= 1);
    assert_eq!(stats.tasks_rejected, 1);
}
