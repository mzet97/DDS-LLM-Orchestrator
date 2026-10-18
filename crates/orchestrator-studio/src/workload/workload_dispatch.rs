//! Parte de `workload` (extraída para manter os arquivos revisáveis).

use super::*;

pub fn dispatch_sync(
    base_url: &str,
    model: &str,
    prompt: &str,
    temperature: f64,
    max_tokens: u32,
    timeout: std::time::Duration,
) -> Result<DispatchOutcome, DispatchError> {
    let url = base_url.trim_end_matches('/');
    let client = reqwest::blocking::Client::builder()
        .timeout(timeout)
        .build()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let body = serde_json::json!({
        "model": model,
        "messages": [{"role": "user", "content": prompt}],
        "temperature": temperature,
        "max_tokens": max_tokens,
    });
    let response = client
        .post(format!("{url}/api/v1/chat/completions/sync"))
        .json(&body)
        .send()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let http_status = response.status().as_u16();
    let headers = response
        .headers()
        .iter()
        .map(|(name, value)| format!("{}: {}", name, value.to_str().unwrap_or("<não-utf8>"),))
        .collect::<Vec<_>>()
        .join("\n");
    let raw = response
        .error_for_status()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?
        .text()
        .map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let value: serde_json::Value =
        serde_json::from_str(&raw).map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    #[derive(Deserialize)]
    struct Outcome {
        task_id: String,
        status: String,
        #[serde(default)]
        assigned_agent: Option<String>,
        #[serde(default)]
        latency_ms: u64,
        #[serde(default)]
        error: Option<String>,
        #[serde(default)]
        content: Option<String>,
        // Decomposição T1–T6 (T-850-04/T6) — 0 quando o backend não reporta.
        #[serde(default)]
        t_serialization_ns: u64,
        #[serde(default)]
        t_transport_send_ns: u64,
        #[serde(default)]
        t_agent_queue_ns: u64,
        #[serde(default)]
        t_inference_ns: u64,
        #[serde(default)]
        t_transport_return_ns: u64,
        #[serde(default)]
        t_deserialization_ns: u64,
    }
    let outcome: Outcome =
        serde_json::from_value(value).map_err(|err| DispatchError::Unreachable {
            url: String::from(url),
            detail: err.to_string(),
        })?;
    let breakdown = LatencyBreakdown::from_ns(
        outcome.t_serialization_ns,
        outcome.t_transport_send_ns,
        outcome.t_agent_queue_ns,
        outcome.t_inference_ns,
        outcome.t_transport_return_ns,
        outcome.t_deserialization_ns,
    );
    let breakdown = breakdown.is_meaningful().then_some(breakdown);
    match outcome.status.as_str() {
        "completed" => Ok(DispatchOutcome::Completed {
            task_id: outcome.task_id,
            assigned_agent: outcome.assigned_agent,
            latency_ms: outcome.latency_ms,
            content: outcome.content,
            breakdown,
            http_status,
            raw,
            headers,
        }),
        "failed" => Ok(DispatchOutcome::Failed {
            task_id: outcome.task_id,
            error: outcome
                .error
                .unwrap_or_else(|| String::from("motivo ausente")),
        }),
        other => Err(DispatchError::Unreachable {
            url: String::from(url),
            detail: format!("status desconhecido: {other}"),
        }),
    }
}
