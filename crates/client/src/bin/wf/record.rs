//! Registro JSON do run para análise posterior (Python só analisa).
//!
//! T-820-11 (notas de semântica do registro):
//! - `status: "completed"` é emitido SOMENTE no caminho de sucesso — `emit`
//!   só é chamado depois que todas as etapas do workflow concluíram com
//!   `success: true` (ver `wf_run.rs`: qualquer `Err` propaga antes de
//!   `emit`). Um run interrompido NÃO gera arquivo com `"status": "failed"`:
//!   simplesmente não gera arquivo (o consumidor deve tratar ausência de
//!   registro como run não-concluído).
//! - Relógios MISTOS, por design: `latency_ms` vem de [`std::time::Instant`]
//!   (monotônico, medido dentro do `client::submit` — válido para comparação
//!   de latências), enquanto `started_ms`/`finished_ms` vêm de relógio de
//!   parede (`SystemTime`) para correlação entre runs/máquinas. As duas
//!   escalas NÃO são subtraíveis entre si (`finished_ms - started_ms` pode
//!   divergir de `latency_ms` sob NTP); cada campo tem seu propósito.

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

/// Serializa o registro do run e grava em `meta.out`.
///
/// T-820-11: escrita via `tokio::fs` — o `std::fs::write` original é
/// bloqueante e rodava dentro de contexto async (`run_seq`/`run_fork`),
/// prendendo a thread do runtime por toda a duração do I/O.
pub async fn emit(
    meta: &RunMeta,
    stages: &[StageRec],
) -> Result<String, Box<dyn std::error::Error>> {
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
    tokio::fs::write(&meta.out, &text).await?;
    Ok(serde_json::to_string(&rec)?)
}
