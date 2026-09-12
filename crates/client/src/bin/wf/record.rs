//! Registro JSON do run para análise posterior (Python só analisa).

pub struct RunMeta {
    pub workflow_id: String,
    pub workload: String,
    pub entry: String,
    pub model: String,
    pub out: String,
}

pub struct StageRec {
    pub stage: &'static str,
    pub task_id: String,
    pub content: String,
    pub expects: Vec<String>,
    pub latency_ms: u64,
    pub started_ms: u64,
    pub finished_ms: u64,
}

pub fn emit(meta: &RunMeta, stages: &[StageRec]) -> Result<String, Box<dyn std::error::Error>> {
    let recs: Vec<serde_json::Value> = stages
        .iter()
        .map(|s| {
            serde_json::json!({
                "stage": s.stage,
                "task_id": s.task_id,
                "content": s.content,
                "expects": s.expects,
                "latency_ms": s.latency_ms,
                "started_ms": s.started_ms,
                "finished_ms": s.finished_ms,
            })
        })
        .collect();
    let rec = serde_json::json!({
        "workflow_id": meta.workflow_id,
        "workload_id": meta.workload,
        "entry": meta.entry,
        "model": meta.model,
        "status": "completed",
        "stages": recs,
    });
    let text = serde_json::to_string_pretty(&rec)?;
    std::fs::write(&meta.out, &text)?;
    Ok(serde_json::to_string(&rec)?)
}
