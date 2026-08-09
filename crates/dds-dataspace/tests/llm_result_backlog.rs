//! Microteste da Fase 3 (plano 9.1): backlog REAL do perfil QoS LLM atual
//! (Reliable 10s + TransientLocal + KeepLast(10) + limits 10/1/10, keyless).
//!
//! Os números daqui dimensionam o novo perfil de `LLM.InferenceResult`
//! (Gate C2: keyless TransientLocal dimensionado por medição). Cada cenário
//! roda em domínio DDS distinto para não conflitar portas.
//!
//! Rode com: `cargo test -p dds-dataspace --features dds --test llm_result_backlog -- --test-threads=1`
#![cfg(feature = "dds")]

use dds_contract::generated::orchestrator::LLMInferenceResult;
use dds_dataspace::api::DataSpaceApi;
use dds_dataspace::DataSpace;
use futures::StreamExt;
use std::collections::HashMap;
use std::time::{Duration, Instant, SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn make_result(request_id: &str, seq: u32, total: u32) -> LLMInferenceResult {
    LLMInferenceResult {
        request_id: request_id.into(),
        seq_num: seq,
        content: format!("chunk-{seq}"),
        is_final: seq + 1 == total,
        finish_reason: if seq + 1 == total { 1 } else { 0 },
        model_used: "microtest".into(),
        tokens_prompt: 8,
        tokens_completion: seq + 1,
        emitted_at_ns: now_ns(),
    }
}

/// Relatório de gaps por stream: (recebidos, faltantes, eventos de gap).
fn gap_report(seqs: &HashMap<String, Vec<u32>>, chunks_per_stream: u32) -> (usize, usize, usize) {
    let mut received = 0usize;
    let mut missing = 0usize;
    let mut gap_events = 0usize;
    for (_id, mut s) in seqs.clone() {
        received += s.len();
        s.sort_unstable();
        let mut expected = 0u32;
        for seq in s {
            if seq > expected {
                gap_events += 1;
                missing += (seq - expected) as usize;
            }
            expected = seq + 1;
        }
        // cauda ausente (final não recebido)
        missing += (chunks_per_stream - expected) as usize;
    }
    (received, missing, gap_events)
}

/// Coleta até `expected` amostras ou o timeout global (nunca loop infinito).
async fn collect_results(
    ds_sub: &DataSpace,
    expected: usize,
    global_timeout: Duration,
    per_sample_delay: Duration,
) -> HashMap<String, Vec<u32>> {
    let mut stream = Box::pin(ds_sub.stream_llm_results());
    let mut seqs: HashMap<String, Vec<u32>> = HashMap::new();
    let start = Instant::now();
    let mut total = 0usize;
    while total < expected && start.elapsed() < global_timeout {
        match tokio::time::timeout(Duration::from_secs(10), stream.next()).await {
            Ok(Some(r)) => {
                seqs.entry(r.request_id.clone())
                    .or_default()
                    .push(r.seq_num);
                total += 1;
                if !per_sample_delay.is_zero() {
                    tokio::time::sleep(per_sample_delay).await;
                }
            }
            Ok(None) => break,
            Err(_) => break, // 10 s sem amostras: encerra com contagem parcial
        }
    }
    seqs
}

/// Publica `streams` × `chunks` resultados; retorna quantos writes falharam
/// (timeout/erro DDS observável no retorno). As streams são intercaladas por
/// rodada de seq_num no mesmo task — `write_llm_result` é `&self` e o
/// DataWriter/RTPS já cuida da entrega concorrente; o que se mede aqui é o
/// histórico GLOBAL keyless sob burst, não paralelismo de chamada.
async fn publish_streams(ds_pub: &DataSpace, streams: u32, chunks: u32, prefix: &str) -> usize {
    let mut write_failures = 0usize;
    for seq_round in 0..chunks {
        for s in 0..streams {
            let req_id = format!("{prefix}-s{s}");
            if ds_pub
                .write_llm_result(make_result(&req_id, seq_round, chunks))
                .await
                .is_err()
            {
                write_failures += 1;
            }
        }
    }
    write_failures
}

/// Cenário A: 1 stream × 128 chunks, reader rápido — baseline sem gaps.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backlog_a_stream_unico_reader_rapido() {
    let ds_pub = DataSpace::new(97, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let ds_sub = DataSpace::new(97, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await; // settle/match

    let chunks = 128u32;
    let collector = tokio::spawn(async move {
        collect_results(&ds_sub, 128, Duration::from_secs(60), Duration::ZERO).await
    });

    let failures = publish_streams(&ds_pub, 1, chunks, "a").await;
    let seqs = collector.await.unwrap();
    let (received, missing, gaps) = gap_report(&seqs, chunks);
    let mut got = seqs.get("a-s0").cloned().unwrap_or_default();
    got.sort_unstable();
    println!("[A] 1x128 reader rápido: recebidos={received} faltantes={missing} gaps={gaps} write_failures={failures}");
    println!("[A] seqs recebidos: {got:?}");
    assert_eq!(failures, 0);
    assert_eq!(received, 128, "baseline não pode perder amostras");
    assert_eq!(missing, 0);
    ds_pub.shutdown().await.unwrap();
}

/// Cenário A2 (diagnóstico): mesmo burst do cenário A, mas com reader DIRETO
/// (polling de `take` a cada 10 ms, SEM SharedWaitSet/stream). Discrimina a
/// camada da perda: 128/128 aqui + perda no A ⇒ o problema está no caminho
/// de notificação (waitset/stream); perda aqui ⇒ camada DDS/QoS.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backlog_a2_reader_direto_sem_waitset() {
    use cyclonedds::{DataReader, DdsEntity, DomainParticipant, Subscriber, Topic};

    let domain = 101;
    let ds_pub = DataSpace::new(domain, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let participant = DomainParticipant::new(domain).expect("participant");
    let subscriber = Subscriber::new(participant.entity()).expect("subscriber");
    let qos = dds_dataspace::qos::profiles::llm_result().expect("qos");
    let topic = Topic::<LLMInferenceResult>::with_qos(
        participant.entity(),
        dds_contract::topics::LLM_RESULT,
        Some(&qos),
    )
    .expect("topic");
    let reader =
        DataReader::<LLMInferenceResult>::with_qos(subscriber.entity(), topic.entity(), Some(&qos))
            .expect("reader");

    tokio::time::sleep(Duration::from_secs(2)).await; // settle/match

    let chunks = 128u32;
    let failures = publish_streams(&ds_pub, 1, chunks, "a2").await;

    // Polling direto: drena o RHC até 128 ou 15 s de silêncio.
    let mut seen: Vec<u32> = Vec::new();
    let mut silence = Duration::ZERO;
    let step = Duration::from_millis(10);
    while seen.len() < chunks as usize && silence < Duration::from_secs(15) {
        let batch = reader.take().expect("take");
        if batch.is_empty() {
            silence += step;
        } else {
            silence = Duration::ZERO;
            for r in batch {
                seen.push(r.seq_num);
            }
        }
        tokio::time::sleep(step).await;
    }
    seen.sort_unstable();
    let mut missing = 0usize;
    let mut expected = 0u32;
    for seq in &seen {
        missing += (seq - expected) as usize;
        expected = seq + 1;
    }
    missing += (chunks - expected) as usize;
    println!(
        "[A2] 1x128 reader DIRETO (sem waitset): recebidos={} faltantes={} write_failures={}",
        seen.len(),
        missing,
        failures
    );
    assert_eq!(
        seen.len(),
        128,
        "reader direto perdeu amostras — camada DDS/QoS"
    );
    ds_pub.shutdown().await.unwrap();
}

/// Cenário B: 1 stream × 128 chunks, reader LENTO (100 ms/amostra).
/// 128 × 100 ms = 12,8 s > max_blocking 10 s do writer — espera-se falha de
/// write por timeout e/ou gaps. É o número que dimensiona o novo perfil.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backlog_b_stream_unico_reader_lento() {
    let ds_pub = DataSpace::new(98, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let ds_sub = DataSpace::new(98, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let chunks = 128u32;
    let collector = tokio::spawn(async move {
        collect_results(
            &ds_sub,
            128,
            Duration::from_secs(90),
            Duration::from_millis(100),
        )
        .await
    });

    let t0 = Instant::now();
    let failures = publish_streams(&ds_pub, 1, chunks, "b").await;
    let publish_elapsed = t0.elapsed();
    let seqs = collector.await.unwrap();
    let (received, missing, gaps) = gap_report(&seqs, chunks);
    println!("[B] 1x128 reader lento 100ms: recebidos={received} faltantes={missing} gaps={gaps} write_failures={failures} publish_em={publish_elapsed:?}");
    // Não asserta zero: o objetivo é MEDIR. Falha só se nada chegar.
    assert!(received > 0, "reader lento não recebeu nada — investigar");
    ds_pub.shutdown().await.unwrap();
}

/// Cenário C: 4 streams × 64 chunks (256 amostras), reader rápido.
/// Histórico keyless é GLOBAL: bursts concorrentes pressionam o RHC do
/// reader (KeepLast(10) na única instância) — gaps aparecem se o take não
/// drenar a tempo.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backlog_c_4_streams_reader_rapido() {
    let ds_pub = DataSpace::new(99, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let ds_sub = DataSpace::new(99, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    tokio::time::sleep(Duration::from_secs(2)).await;

    let collector = tokio::spawn(async move {
        collect_results(&ds_sub, 256, Duration::from_secs(60), Duration::ZERO).await
    });

    let failures = publish_streams(&ds_pub, 4, 64, "c").await;
    let seqs = collector.await.unwrap();
    let (received, missing, gaps) = gap_report(&seqs, 64);
    println!("[C] 4x64 reader rápido: recebidos={received} faltantes={missing} gaps={gaps} write_failures={failures} streams_vistas={}", seqs.len());
    assert!(received > 0);
    ds_pub.shutdown().await.unwrap();
}

/// Cenário D2 (diagnóstico): late joiner com reader DIRETO (sem stream/cache)
/// criado DEPOIS da publicação. Isola a entrega histórica TransientLocal na
/// camada DDS: 64/64 aqui + 1 no D ⇒ perda está no stream; ~1 aqui ⇒ a QoS
/// TransientLocal não está retendo/entregando histórico como esperado.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backlog_d2_late_joiner_reader_direto() {
    use cyclonedds::{DataReader, DdsEntity, DomainParticipant, Subscriber, Topic};

    let domain = 102;
    let ds_pub = DataSpace::new(domain, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let failures = publish_streams(&ds_pub, 1, 64, "d2").await;
    assert_eq!(failures, 0);

    // Reader nasce depois de tudo publicado, direto na API cyclonedds.
    let participant = DomainParticipant::new(domain).expect("participant");
    let subscriber = Subscriber::new(participant.entity()).expect("subscriber");
    let qos = dds_dataspace::qos::profiles::llm_result().expect("qos");
    let topic = Topic::<LLMInferenceResult>::with_qos(
        participant.entity(),
        dds_contract::topics::LLM_RESULT,
        Some(&qos),
    )
    .expect("topic");
    let reader =
        DataReader::<LLMInferenceResult>::with_qos(subscriber.entity(), topic.entity(), Some(&qos))
            .expect("reader");

    let mut seen: Vec<u32> = Vec::new();
    let mut silence = Duration::ZERO;
    let step = Duration::from_millis(50);
    while seen.len() < 64 && silence < Duration::from_secs(10) {
        let batch = reader.take().expect("take");
        if batch.is_empty() {
            silence += step;
        } else {
            silence = Duration::ZERO;
            for r in batch {
                seen.push(r.seq_num);
            }
        }
        tokio::time::sleep(step).await;
    }
    seen.sort_unstable();
    println!(
        "[D2] late joiner DIRETO após 64 chunks: recebidos={} seqs={:?}",
        seen.len(),
        seen
    );
    ds_pub.shutdown().await.unwrap();
}

/// Cenário D3 (diagnóstico): retenção TransientLocal com reader MATCHED na
/// escrita. reader1 existe durante a publicação (drena tudo); reader2 nasce
/// depois. Se reader2 receber 64/64, a retenção funciona quando havia reader
/// matched na escrita — e o problema do D2 é específico de "zero readers".
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backlog_d3_retencao_com_reader_matched() {
    use cyclonedds::{DataReader, DdsEntity, DomainParticipant, Subscriber, Topic};

    let domain = 103;
    let ds_pub = DataSpace::new(domain, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let participant = DomainParticipant::new(domain).expect("participant");
    let subscriber = Subscriber::new(participant.entity()).expect("subscriber");
    let qos = dds_dataspace::qos::profiles::llm_result().expect("qos");
    let topic = Topic::<LLMInferenceResult>::with_qos(
        participant.entity(),
        dds_contract::topics::LLM_RESULT,
        Some(&qos),
    )
    .expect("topic");

    // reader1: matched ANTES da publicação.
    let reader1 =
        DataReader::<LLMInferenceResult>::with_qos(subscriber.entity(), topic.entity(), Some(&qos))
            .expect("reader1");
    tokio::time::sleep(Duration::from_secs(2)).await; // settle/match

    let failures = publish_streams(&ds_pub, 1, 64, "d3").await;
    assert_eq!(failures, 0);

    // drena reader1 (confirma entrega ao vivo)
    let mut live = 0usize;
    let mut silence = Duration::ZERO;
    let step = Duration::from_millis(50);
    while live < 64 && silence < Duration::from_secs(10) {
        let batch = reader1.take().expect("take");
        if batch.is_empty() {
            silence += step;
        } else {
            silence = Duration::ZERO;
            live += batch.len();
        }
        tokio::time::sleep(step).await;
    }

    // reader2: late joiner com a WHC "quente".
    let reader2 =
        DataReader::<LLMInferenceResult>::with_qos(subscriber.entity(), topic.entity(), Some(&qos))
            .expect("reader2");
    let mut seen2 = 0usize;
    let mut silence2 = Duration::ZERO;
    while seen2 < 64 && silence2 < Duration::from_secs(10) {
        let batch = reader2.take().expect("take");
        if batch.is_empty() {
            silence2 += step;
        } else {
            silence2 = Duration::ZERO;
            seen2 += batch.len();
        }
        tokio::time::sleep(step).await;
    }
    println!("[D3] reader1 (live)={live}/64, reader2 (late joiner pós-match)={seen2}/64");
    ds_pub.shutdown().await.unwrap();
}

/// Cenário D: late joiner — 64 chunks publicados ANTES do reader existir.
/// TransientLocal entrega o histórico retido: com KeepLast(10) espera-se
/// exatamente 10 (as mais recentes). Mede a retenção real para o novo perfil.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn backlog_d_late_joiner() {
    let ds_pub = DataSpace::new(100, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    tokio::time::sleep(Duration::from_secs(1)).await;

    let failures = publish_streams(&ds_pub, 1, 64, "d").await;
    assert_eq!(failures, 0);

    // Reader nasce depois de tudo publicado.
    let ds_sub = DataSpace::new(100, DataSpace::STRENGTH_ORCHESTRATOR).unwrap();
    let seqs = collect_results(&ds_sub, 64, Duration::from_secs(15), Duration::ZERO).await;
    let (received, _missing, _gaps) = gap_report(&seqs, 64);
    println!("[D] late joiner após 64 chunks: recebidos={received} (KeepLast(10) → esperado ~10)");
    assert!(
        received > 0,
        "TransientLocal não entregou histórico ao late joiner"
    );
    ds_pub.shutdown().await.unwrap();
    ds_sub.shutdown().await.unwrap();
}

/// Probe FFI-QC-010E: readcondition sobre tópico de PRODUÇÃO (Task, gerado
/// pelo cyclonedds-idlc COM metadata XTypes). Se funcionar, o BadParameter
/// dos testes da binding é limitado a tipos legacy-ops sem metadata.
#[test]
fn probe_readcondition_em_topico_idlc() {
    use cyclonedds::{DataReader, DdsEntity, DomainParticipant, Subscriber, Topic};
    use dds_contract::generated::dds_llm_orchestrator::Task;

    let participant = DomainParticipant::new(105).expect("participant");
    let subscriber = Subscriber::new(participant.entity()).expect("subscriber");
    let qos = dds_dataspace::qos::profiles::tasks(None).expect("qos");
    let topic = Topic::<Task>::with_qos(
        participant.entity(),
        dds_contract::topics::TASKS,
        Some(&qos),
    )
    .expect("topic");
    let reader = DataReader::<Task>::with_qos(subscriber.entity(), topic.entity(), Some(&qos))
        .expect("reader");
    let mask = 3u32 | 12 | 112; // ANY sample/view/instance
    let rc = unsafe {
        cyclonedds_rust_sys::dds_create_readcondition(reader.entity(), mask)
    };
    println!("[probe] readcondition em tópico idlc (Tasks) = {rc}");
    assert!(rc > 0, "readcondition falhou em tópico idlc de produção");
}
