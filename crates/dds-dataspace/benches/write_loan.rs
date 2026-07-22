//! Microbenchmark (Fase R3 do `OPTIMIZATION_PLAN.md`): `DataWriter::write`
//! (via `WriteArena`/`write_to_native`) vs `write_output_loan` (zero-copy,
//! Fase 4) para `TaskOutput` — o tópico de maior volume de samples por
//! sessão de inferência (um por chunk de streaming).
//!
//! A Fase 4 já validou a correção por corretude (round-trip de 1000 chunks,
//! 0 gaps) — este benchmark mede a magnitude do ganho de tempo/CPU por
//! escrita, que nunca tinha sido medida.
//!
//! Rode com: `CYCLONEDDS_STATIC=1 cargo bench -p dds-dataspace --features dds --bench write_loan`
#![cfg(feature = "dds")]

use criterion::{criterion_group, criterion_main, Criterion};
use cyclonedds::{DataWriter, DdsEntity, DomainParticipant, Publisher, Topic};
use dds_contract::generated::dds_llm_orchestrator::TaskOutput;
use dds_dataspace::writer_pool::write_output_loan;
use std::hint::black_box;
use std::time::{SystemTime, UNIX_EPOCH};

const DOMAIN: u32 = 63; // distinto do domain 62 usado por spike-interop/benches/roundtrip.rs

struct Fixture {
    _dp: DomainParticipant,
    _topic: Topic<TaskOutput>,
    _pub: Publisher,
    writer: DataWriter<TaskOutput>,
}

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64
}

fn setup() -> Fixture {
    let dp = DomainParticipant::new(DOMAIN).expect("participant");
    let topic =
        Topic::<TaskOutput>::new(dp.entity(), "bench.write_loan.TaskOutput").expect("topic");
    let publisher = Publisher::new(dp.entity()).expect("publisher");
    let writer = DataWriter::with_qos(publisher.entity(), topic.entity(), None).expect("writer");
    Fixture {
        _dp: dp,
        _topic: topic,
        _pub: publisher,
        writer,
    }
}

fn make_output(seq: u32) -> TaskOutput {
    TaskOutput {
        task_id: "bench-write-loan-task".into(),
        seq_num: seq,
        content: format!("chunk-{seq}-conteudo-representativo-de-um-token-de-inferencia"),
        is_final: false,
        finish_reason: 0,
        agent_id: "bench-agent".into(),
        token_count: seq + 1,
        emitted_at_ns: now_ns(),
    }
}

fn bench_write_copy(c: &mut Criterion) {
    let fx = setup();
    let mut seq = 0u32;
    c.bench_function("task_output_write_copy", |b| {
        b.iter(|| {
            let out = make_output(seq);
            seq = seq.wrapping_add(1);
            black_box(fx.writer.write(black_box(&out))).expect("write");
        });
    });
}

fn bench_write_loan(c: &mut Criterion) {
    let fx = setup();
    let mut seq = 0u32;
    c.bench_function("task_output_write_loan_zero_copy", |b| {
        b.iter(|| {
            let out = make_output(seq);
            seq = seq.wrapping_add(1);
            black_box(write_output_loan(&fx.writer, black_box(&out))).expect("write_output_loan");
        });
    });
}

criterion_group!(benches, bench_write_copy, bench_write_loan);
criterion_main!(benches);
