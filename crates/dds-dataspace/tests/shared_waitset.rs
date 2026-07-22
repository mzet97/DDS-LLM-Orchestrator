//! Teste de aceite da Fase 5 (T-617): WaitSet compartilhado.
//!
//! Antes desta mudança, cada `stream_*()` criava seu PRÓPRIO `WaitSet` (via
//! `take_aiter()`), ocupando uma thread do blocking-pool do tokio por toda a
//! vida da stream. O padrão real mais exigente do workspace é o `client`:
//! cada `submit()` concorrente abre 2 streams (`stream_tasks` +
//! `stream_task_outputs`) — com 50 clientes concorrentes (já validado em
//! `specs/300-control-plane`), isso eram até 100 WaitSets simultâneos.
//!
//! Este teste prova, com DDS real (não mock):
//! 1. N streams de `Task` + N de `TaskOutput` concorrentes compartilham o
//!    MESMO `SharedWaitSet` (2N registros num único WaitSet, não 2N WaitSets).
//! 2. Cada stream, apesar de compartilhar o mecanismo de espera, continua
//!    recebendo TODAS as amostras de forma independente (a semântica de
//!    N assinantes por tópico — cada um com seu próprio reader/`dds_take` —
//!    não foi alterada por compartilhar só o WaitSet).
//! 3. Ao dropar as streams, os registros são liberados (sem vazamento).
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::sync::Arc;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 86;
const N: usize = 20;

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn make_task(id: &str) -> Task {
    Task {
        task_id: id.into(),
        client_id: "shared-waitset-test".into(),
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
        created_at_ns: now_ns(),
        assigned_at_ns: 0,
        started_at_ns: 0,
        completed_at_ns: 0,
        deadline_ns: now_ns() + 60_000_000_000,
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

/// N streams de `Task` + N de `TaskOutput` concorrentes: 2N registros em UM
/// `SharedWaitSet` (não 2N WaitSets independentes), cada stream ainda vendo
/// todas as amostras, e limpeza correta ao dropar.
#[tokio::test(flavor = "multi_thread", worker_threads = 8)]
async fn n_concurrent_streams_share_one_waitset_and_still_see_everything() {
    let ds = Arc::new(DataSpace::new(DOMAIN, DataSpace::STRENGTH_ORCHESTRATOR).unwrap());

    assert_eq!(
        ds.shared_waitset().registration_count(),
        0,
        "sem streams abertas, 0 registros esperados"
    );

    // Abre N streams de Tasks + N de TaskOutput concorrentes, cada uma
    // consumida em sua própria task (mesmo padrão do client real: cada
    // submit() abre 1 de cada — client/src/lib.rs).
    let mut task_handles = Vec::new();
    let mut received_counts = Vec::new();
    for _ in 0..N {
        let ds = Arc::clone(&ds);
        let (tx, rx) = tokio::sync::oneshot::channel();
        received_counts.push(rx);
        task_handles.push(tokio::spawn(async move {
            let mut s = Box::pin(ds.stream_tasks());
            let mut count = 0usize;
            // Cada stream deve ver TODAS as N tasks publicadas (assinantes
            // independentes, não fan-out/broadcast).
            while count < N {
                match tokio::time::timeout(Duration::from_secs(15), s.next()).await {
                    Ok(Some(_)) => count += 1,
                    _ => break,
                }
            }
            let _ = tx.send(count);
        }));
    }

    // Também abre N streams de TaskOutput concorrentes (só para inflar a
    // contagem de registros no SharedWaitSet igual ao padrão real do client
    // — não avaliamos o conteúdo dessas aqui, só que coexistem no mesmo
    // WaitSet).
    let mut output_handles = Vec::new();
    for _ in 0..N {
        let ds = Arc::clone(&ds);
        output_handles.push(tokio::spawn(async move {
            let mut s = Box::pin(ds.stream_task_outputs());
            let _ = tokio::time::timeout(Duration::from_millis(500), s.next()).await;
        }));
    }

    // Settle: dá tempo de cada stream ser agendada e registrar no WaitSet
    // compartilhado (a 1ª poll de cada stream roda `waitset.register(..)`).
    tokio::time::sleep(Duration::from_millis(300)).await;

    assert_eq!(
        ds.shared_waitset().registration_count(),
        2 * N,
        "esperava {} registros (N streams de Tasks + N de TaskOutput) no MESMO SharedWaitSet",
        2 * N
    );

    // Publica N tasks reais via DDS — cada uma das N streams de stream_tasks
    // deve ver todas as N (assinantes independentes).
    for i in 0..N {
        ds.write_task_sync(&make_task(&format!("shared-waitset-{i}")))
            .expect("write_task_sync");
    }

    for rx in received_counts {
        let count = rx.await.expect("stream task não terminou");
        assert_eq!(
            count, N,
            "cada stream_tasks() deveria ver todas as {N} tasks publicadas (assinante independente)"
        );
    }
    for h in task_handles {
        h.await.unwrap();
    }
    for h in output_handles {
        h.await.unwrap();
    }

    println!(
        "[Fase 5] {} streams concorrentes (Tasks+TaskOutput) compartilharam 1 SharedWaitSet \
         ({} registros de pico); cada stream_tasks() viu as {N} tasks publicadas",
        2 * N,
        2 * N
    );

    // Todas as streams já terminaram (loop saiu ao ver N tasks, ou timeout) —
    // os `Registration` foram dropados: registros devem ter sido liberados.
    tokio::time::sleep(Duration::from_millis(300)).await;
    assert_eq!(
        ds.shared_waitset().registration_count(),
        0,
        "registros deveriam ter sido liberados (RAII) ao dropar as streams"
    );

    let ds = Arc::try_unwrap(ds)
        .ok()
        .expect("DataSpace ainda tem outras referências Arc vivas");
    ds.shutdown().await.unwrap();
}
