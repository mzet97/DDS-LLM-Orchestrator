//! T-900-M6 — CLI de validação de candidatos de plano (campanha M6).
//!
//! stdin: {"intent": {...WorkflowIntent mock...}, "candidate": {...PlanCandidate...}}
//! stdout: {"valid": true|false, "reason": "..."}
//! exit: 0 válido / 1 inválido / 2 erro de uso. Com feature `dds` os tipos de
//! tópico trocam de representação — o bin exige o build padrão (mock).

fn main() {
    #[cfg(feature = "dds")]
    {
        eprintln!("dag-validate: compile sem a feature `dds` (representação mock)");
        std::process::exit(2);
    }
    #[cfg(not(feature = "dds"))]
    {
        run();
    }
}

#[cfg(not(feature = "dds"))]
fn run() {
    use dds_contract::workflow::dds_llm_workflow_v1::WorkflowIntent;
    use std::io::Read;
    use workflow_engine::dag::validate_candidate;

    let mut buf = String::new();
    std::io::stdin()
        .read_to_string(&mut buf)
        .expect("ler stdin");
    let parsed: serde_json::Value = serde_json::from_str(&buf).expect("JSON de entrada");
    let intent: WorkflowIntent =
        serde_json::from_value(parsed["intent"].clone()).expect("intent malformada");
    let candidate: workflow_engine::dag::PlanCandidate =
        serde_json::from_value(parsed["candidate"].clone()).expect("candidate malformado");
    let now = std::time::SystemTime::now()
        .duration_since(std::time::UNIX_EPOCH)
        .unwrap_or_default()
        .as_nanos() as u64;
    let verdict = match validate_candidate(&candidate, &intent, now) {
        Ok(()) => serde_json::json!({"valid": true, "nodes": candidate.nodes.len()}),
        Err(e) => serde_json::json!({"valid": false, "reason": e.to_string()}),
    };
    println!("{verdict}");
    std::process::exit(if verdict["valid"].as_bool().unwrap_or(false) {
        0
    } else {
        1
    });
}
