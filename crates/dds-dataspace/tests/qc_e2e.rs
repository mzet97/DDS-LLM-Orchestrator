//! E2E FFI-QC-010: QueryCondition com closure em tópico de produção (Tasks,
//! tipo idlc com metadata XTypes — o único caminho em que conditions
//! funcionam no CycloneDDS 11.0.1; ver achado FFI-QC-010E na matriz).
//!
//! Rode com: `cargo test -p dds-dataspace --features dds --test qc_e2e -- --test-threads=1 --nocapture`
#![cfg(feature = "dds")]

use cyclonedds::{
    DataReader, DataWriter, DdsEntity, DomainParticipant, Publisher, QueryCondition, Subscriber,
    Topic,
};
use dds_contract::generated::dds_llm_orchestrator::Task;
use std::time::{Duration, SystemTime, UNIX_EPOCH};

fn now_ns() -> u64 {
    SystemTime::now()
        .duration_since(UNIX_EPOCH)
        .unwrap()
        .as_nanos() as u64
}

fn make_task(id: &str) -> Task {
    Task {
        task_id: id.into(),
        client_id: "qc-e2e".into(),
        assigned_agent: String::new(),
        target_agent: String::new(),
        model_required: 0,
        model_name: "m".into(),
        messages_json: "[]".into(),
        temperature: 0.7,
        max_tokens: 8,
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

fn take_via_condition(qc: &QueryCondition, max: usize) -> usize {
    let mut samples: Vec<*mut std::ffi::c_void> = vec![std::ptr::null_mut(); max];
    let mut infos: Vec<cyclonedds_rust_sys::dds_sample_info> =
        (0..max).map(|_| unsafe { std::mem::zeroed() }).collect();
    let n = unsafe {
        cyclonedds_rust_sys::dds_take(
            qc.entity(),
            samples.as_mut_ptr(),
            infos.as_mut_ptr() as *mut _,
            max,
            max as u32,
        )
    };
    assert!(n >= 0, "dds_take falhou: {n}");
    let valid = (0..n as usize)
        .filter(|&i| infos[i].valid_data && !samples[i].is_null())
        .count();
    unsafe {
        cyclonedds_rust_sys::dds_return_loan(qc.entity(), samples.as_mut_ptr(), n as i32);
    }
    valid
}

const MASK: u32 = 3 | 12 | 112; // ANY sample/view/instance

#[test]
fn query_condition_e2e_topico_producao() {
    let participant = DomainParticipant::new(106).expect("participant");
    let publisher = Publisher::new(participant.entity()).expect("publisher");
    let subscriber = Subscriber::new(participant.entity()).expect("subscriber");
    let writer_qos = dds_dataspace::qos::profiles::tasks(Some(100)).expect("writer qos");
    let reader_qos = dds_dataspace::qos::profiles::tasks(None).expect("reader qos");
    let topic = Topic::<Task>::with_qos(
        participant.entity(),
        dds_contract::topics::TASKS,
        Some(&reader_qos),
    )
    .expect("topic");
    let writer =
        DataWriter::<Task>::with_qos(publisher.entity(), topic.entity(), Some(&writer_qos))
            .expect("writer");
    let reader =
        DataReader::<Task>::with_qos(subscriber.entity(), topic.entity(), Some(&reader_qos))
            .expect("reader");

    for i in 0..3 {
        writer.write(&make_task(&format!("qc-{i}"))).expect("write");
    }
    std::thread::sleep(Duration::from_millis(700));

    // 1. Filtro accept-all COM guard: as 3 amostras chegam.
    let calls = std::sync::Arc::new(std::sync::atomic::AtomicUsize::new(0));
    let calls2 = std::sync::Arc::clone(&calls);
    let qc_all = QueryCondition::with_filter(reader.entity(), MASK, move |_| {
        calls2.fetch_add(1, std::sync::atomic::Ordering::Relaxed);
        true
    })
    .expect("qc all");
    // 1b. Caminho WAITSET (o uso primário de QC): attach + wait dispara?
    let waitset = cyclonedds::WaitSet::new(participant.entity()).expect("waitset");
    waitset.attach(qc_all.entity(), 77).expect("attach");
    let triggered = {
        let _g = qc_all.activate();
        waitset.wait(2_000_000_000).expect("wait")
    };
    println!(
        "[qc_e2e] waitset+QC: triggered={triggered:?}, filter invocado {} vez(es)",
        calls.load(std::sync::atomic::Ordering::Relaxed)
    );
    // 1c. ReadCondition simples (sem filtro) no mesmo reader: discrimina
    // "conditions quebradas em geral" vs "só querycondition com closure".
    let rc_any = cyclonedds::ReadCondition::any(reader.entity()).expect("readcondition");
    let n_rc = {
        let mut samples: Vec<*mut std::ffi::c_void> = vec![std::ptr::null_mut(); 8];
        let mut infos: Vec<cyclonedds_rust_sys::dds_sample_info> =
            (0..8).map(|_| unsafe { std::mem::zeroed() }).collect();
        let n = unsafe {
            cyclonedds_rust_sys::dds_take(
                rc_any.entity(),
                samples.as_mut_ptr(),
                infos.as_mut_ptr() as *mut _,
                8,
                8,
            )
        };
        if n > 0 {
            unsafe {
                cyclonedds_rust_sys::dds_return_loan(rc_any.entity(), samples.as_mut_ptr(), n);
            }
        }
        n
    };
    println!("[qc_e2e] take via ReadCondition(simples) = {n_rc}");

    // 1d. QueryCondition com filtro C estático (sem trampoline): discrimina
    // "trampoline Rust quebrado" vs "QC com filtro quebrado na biblioteca".
    unsafe extern "C" fn accept_all_c(_s: *const std::ffi::c_void) -> bool {
        true
    }
    let qc_c = QueryCondition::new(reader.entity(), MASK, accept_all_c).expect("qc c-filter");
    let n_c = take_via_condition(&qc_c, 8);
    println!("[qc_e2e] take via QC com filtro C estático = {n_c}");

    let n = {
        let _g = qc_all.activate();
        take_via_condition(&qc_all, 8)
    };

    // ACHADO FFI-QC-010E (medido 2026-08-08, CycloneDDS 11.0.1):
    // - ReadCondition simples: FUNCIONA (take via condition retorna as 3).
    // - QueryCondition com filtro (closure Rust OU filtro C estático): NUNCA
    //   é avaliada — filtro não invocado, take retorna 0, waitset não
    //   dispara. Quebrado no nível da biblioteca, não no trampoline.
    // Decisão do plano (9.3): manter filtro por request_id NA APLICAÇÃO —
    // agora justificado por medição. QCs não são usadas em produção.
    assert_eq!(n_rc, 3, "ReadCondition simples deve entregar as 3 amostras");
    assert_eq!(
        n_c, 0,
        "QC com filtro C: comportamento quebrado documentado"
    );
    assert_eq!(n, 0, "QC com closure: comportamento quebrado documentado");
    println!("[qc_e2e] via QC(condition)={n} — QC-filtro quebrada na 11.0.1 (documentado)");
}
