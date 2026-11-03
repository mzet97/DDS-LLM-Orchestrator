//! Repro do crash de heap em `dds_write` (Fase 0b, debug de layout de tipo).
//!
//! Roda 3 experimentos, imprimindo OK após cada um:
//!   A) struct mínima { #[key] id: String, val: u32 } — SEM repr(C)
//!   B) espelho de Task (todos os campos)            — SEM repr(C)
//!   C) espelho de Task (todos os campos)            — COM repr(C)
//!
//! Se o crash ocorre em A/B mas não em C → o descritor do derive exige repr(C)
//! e o codegen do cyclonedds-build precisa emiti-lo.

use cyclonedds::DdsTypeDerive;
use cyclonedds::{DataWriter, DdsType, DomainParticipant, Publisher, Topic};
use std::io::Write as _;

#[derive(Debug, Clone, DdsTypeDerive)]
#[dds_typename("repro::MinimalNoRepr")]
struct MinimalNoRepr {
    #[key]
    id: String,
    val: u32,
}

#[derive(Debug, Clone, DdsTypeDerive)]
#[dds_typename("repro::TaskNoRepr")]
struct TaskNoRepr {
    #[key]
    task_id: String,
    client_id: String,
    assigned_agent: String,
    model_required: i32,
    model_name: String,
    messages_json: String,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
    status: i32,
    priority: i32,
    created_at_ns: u64,
    assigned_at_ns: u64,
    started_at_ns: u64,
    completed_at_ns: u64,
    deadline_ns: u64,
    retry_count: u32,
    finish_reason: String,
}

#[repr(C)]
#[derive(Debug, Clone, DdsTypeDerive)]
#[dds_typename("repro::TaskReprC")]
struct TaskReprC {
    #[key]
    task_id: String,
    client_id: String,
    assigned_agent: String,
    model_required: i32,
    model_name: String,
    messages_json: String,
    temperature: f32,
    max_tokens: u32,
    stream: bool,
    status: i32,
    priority: i32,
    created_at_ns: u64,
    assigned_at_ns: u64,
    started_at_ns: u64,
    completed_at_ns: u64,
    deadline_ns: u64,
    retry_count: u32,
    finish_reason: String,
}

fn make_task_fields() -> (String, String, String, String, String) {
    (
        "repro-task-0000".into(),
        "repro-client".into(),
        "".into(),
        "repro-model".into(),
        r#"[{"role":"user","content":"repro"}]"#.into(),
    )
}

fn write_one<T: DdsType>(dp: &DomainParticipant, topic_name: &str, v: &T) -> anyhow::Result<()> {
    let topic = Topic::<T>::with_qos(dp, topic_name, None)?;
    let publisher = Publisher::new(dp)?;
    let writer = DataWriter::new(&publisher, &topic)?;
    writer.write(v)?;
    Ok(())
}

fn step(name: &str) {
    print!("{name}... ");
    std::io::stdout().flush().unwrap();
}

fn ok() {
    println!("OK");
    std::io::stdout().flush().unwrap();
}

fn main() -> anyhow::Result<()> {
    let dp = DomainParticipant::new(60)?;
    let (task_id, client_id, assigned_agent, model_name, messages_json) = make_task_fields();

    step("A) MinimalNoRepr { #[key] String, u32 }");
    write_one(
        &dp,
        "ReproMinimal",
        &MinimalNoRepr {
            id: "repro-0".into(),
            val: 42,
        },
    )?;
    ok();

    step("B) TaskNoRepr (18 campos, sem repr(C)) — PULADO (crash conhecido, isola C)");
    let skip_b = std::env::args().any(|a| a == "--skip-b");
    if !skip_b {
        write_one(
            &dp,
            "ReproTaskNoRepr",
            &TaskNoRepr {
                task_id: task_id.clone(),
                client_id: client_id.clone(),
                assigned_agent: assigned_agent.clone(),
                model_required: 0,
                model_name: model_name.clone(),
                messages_json: messages_json.clone(),
                temperature: 0.7,
                max_tokens: 8,
                stream: false,
                status: 0,
                priority: 1,
                created_at_ns: 1,
                assigned_at_ns: 0,
                started_at_ns: 0,
                completed_at_ns: 0,
                deadline_ns: 60_000_000_000,
                retry_count: 0,
                finish_reason: String::new(),
            },
        )?;
    }
    ok();

    step("C) TaskReprC (18 campos, com repr(C))");
    write_one(
        &dp,
        "ReproTaskReprC",
        &TaskReprC {
            task_id,
            client_id,
            assigned_agent,
            model_required: 0,
            model_name,
            messages_json,
            temperature: 0.7,
            max_tokens: 8,
            stream: false,
            status: 0,
            priority: 1,
            created_at_ns: 1,
            assigned_at_ns: 0,
            started_at_ns: 0,
            completed_at_ns: 0,
            deadline_ns: 60_000_000_000,
            retry_count: 0,
            finish_reason: String::new(),
        },
    )?;
    ok();

    println!("FIM: todos os experimentos passaram");
    Ok(())
}
