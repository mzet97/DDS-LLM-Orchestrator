//! Regressão de soundness: `take_aiter`/`take_async` com tipo contendo `String`.
//!
//! Antes do fix em `cyclonedds/src/async.rs`, o caminho async fazia
//! `std::ptr::read` na amostra NATIVA (layout C: strings = `char*` de 8 bytes)
//! reinterpretando-a como o struct Rust (`String` de 24 bytes) — UB que lia
//! len/cap como lixo e liberava ponteiros arbitrários ao dropar. O fix usa
//! `T::clone_out` (como o caminho síncrono). Este teste falha/crashava antes
//! do fix e deve passar para sempre.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo test -p dds-contract --features dds -- --test-threads=1`
#![cfg(feature = "dds")]

use cyclonedds::{
    DataReader, DataWriter, DomainParticipant, Durability, History, Ownership, Publisher,
    QosBuilder, Subscriber, Topic,
};
use dds_contract::generated::dds_llm_orchestrator::Task;
use dds_contract::topics;
use futures::StreamExt;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

// Cada teste usa um domínio próprio: `cargo test` roda as funções deste binário
// em paralelo e, com `TransientLocal`, um reader enxergaria as amostras escritas
// pela outra função no mesmo domínio/tópico (contagem não-determinística).
const DOMAIN_AITER: u32 = 80;
const DOMAIN_ASYNC: u32 = 81;

fn qos() -> cyclonedds::Qos {
    QosBuilder::new()
        .reliable()
        .durability(Durability::TransientLocal)
        .history(History::KeepLast(50))
        .ownership(Ownership::Exclusive)
        .build()
        .unwrap()
}

fn make_task(i: usize) -> Task {
    let now_ns = SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64;
    Task {
        task_id: format!("soundness-task-{i:04}"),
        client_id: "soundness-client".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "qwen3.5-0.8b".into(),
        messages_json: format!(r#"[{{"role":"user","content":"msg {i}"}}]"#),
        temperature: 0.7,
        max_tokens: 64,
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

fn setup(
    domain: u32,
) -> (
    DomainParticipant,
    Topic<Task>,
    Publisher,
    Subscriber,
    DataWriter<Task>,
    DataReader<Task>,
) {
    let dp = DomainParticipant::new(domain).unwrap();
    let q = qos();
    let topic = Topic::<Task>::with_qos(&dp, topics::TASKS, Some(&q)).unwrap();
    let publisher = Publisher::new(&dp).unwrap();
    let writer = DataWriter::with_qos(&publisher, &topic, Some(&q)).unwrap();
    let subscriber = Subscriber::new(&dp).unwrap();
    let reader = DataReader::with_qos(&subscriber, &topic, Some(&q)).unwrap();
    // Tópico/pub/sub precisam ficar vivos — dropá-los invalida writer/reader.
    (dp, topic, publisher, subscriber, writer, reader)
}

/// 50 Tasks com strings via take_aiter: conteúdo deve bater exatamente.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn take_aiter_with_string_fields_is_sound() {
    let (_dp, _t, _p, _s, writer, reader) = setup(DOMAIN_AITER);
    let n = 50;

    // Settle para o match reader↔writer (mesmo processo, dominio exclusivo).
    tokio::time::sleep(Duration::from_millis(1500)).await;

    let writer_handle = tokio::task::spawn_blocking(move || {
        for i in 0..n {
            writer.write(&make_task(i)).unwrap();
        }
        writer
    });

    let mut received: Vec<Task> = Vec::new();
    let mut stream = Box::pin(reader.take_aiter_timeout(500_000_000));
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while received.len() < n && tokio::time::Instant::now() < deadline {
        match tokio::time::timeout(Duration::from_secs(2), stream.next()).await {
            Ok(Some(Ok(batch))) => received.extend(batch),
            Ok(Some(Err(e))) => panic!("erro no stream: {e}"),
            _ => continue,
        }
    }
    writer_handle.await.unwrap();

    assert_eq!(
        received.len(),
        n,
        "esperava {n} tasks, recebeu {}",
        received.len()
    );
    for (i, t) in received.iter().enumerate() {
        assert!(
            t.task_id.starts_with("soundness-task-"),
            "task_id corrompido: {:?}",
            t.task_id
        );
        assert_eq!(t.model_name, "qwen3.5-0.8b");
        assert_eq!(
            t.messages_json.len(),
            34 + i.to_string().len(),
            "messages_json com tamanho errado (len/cap lixo?): {:?}",
            t.messages_json
        );
        assert!(t.created_at_ns > 0);
    }
}

/// take_async (spawn_blocking) com strings: idem.
#[tokio::test(flavor = "multi_thread", worker_threads = 4)]
async fn take_async_with_string_fields_is_sound() {
    let (_dp, _t, _p, _s, writer, reader) = setup(DOMAIN_ASYNC);
    let n = 30;

    tokio::time::sleep(Duration::from_millis(1500)).await;

    let writer_handle = tokio::task::spawn_blocking(move || {
        for i in 0..n {
            writer.write(&make_task(i)).unwrap();
        }
        writer
    });

    let mut received: Vec<Task> = Vec::new();
    let deadline = tokio::time::Instant::now() + Duration::from_secs(20);
    while received.len() < n && tokio::time::Instant::now() < deadline {
        let batch = reader.take_async().await.unwrap();
        received.extend(batch);
        tokio::time::sleep(Duration::from_millis(10)).await;
    }
    writer_handle.await.unwrap();

    assert_eq!(received.len(), n);
    for t in &received {
        assert!(t.task_id.starts_with("soundness-task-"));
        assert_eq!(t.client_id, "soundness-client");
        assert!(t.messages_json.starts_with("[{\"role\""));
    }
}
