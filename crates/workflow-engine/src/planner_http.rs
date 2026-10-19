//! Planner LLM via HTTP (llama-server local) — T-900-E2E-B1.
//!
//! D7: planner GENERATIVO separado do Laya; usa o servidor llama.cpp local
//! (openai-compatible), mesmo modelo oficial do B-BENCH. Espelho Rust do
//! `tools/llm_planner.py`: max_tokens 2048 (o gemma-4 raciocina muito e o
//! `content` sai vazio com orçamento pequeno), temperatura 0, fallback de
//! extração no `reasoning_content`.
//!
//! `plan()` é SYNC (trait `Planner`) e é chamada de dentro do runtime tokio
//! do wf-run: a chamada HTTP roda numa THREAD DEDICADA com runtime
//! single-thread próprio (`block_on` no reactor panica).

use crate::dag::{CandidateNode, NodeKindCand, PlanCandidate};
use crate::engine::Planner;

pub const PLANNER_SYSTEM: &str = "You are a task planner. Decompose the user request into 1 to 4 subtasks. Reply ONLY with a JSON array; each element: {\"node_id\": \"n1\", \"dependencies\": [], \"required_capabilities\": [\"...\"], \"prompt\": \"...\"}. Allowed capabilities: rust_refactor, python_refactor, unit_testing, security_audit, code_review, research, summarization, translation. Use an empty dependency list for the first task and node ids of earlier tasks for later ones. Single-step requests need exactly one node.";

pub struct LlmHttpPlanner {
    url: String,
}

impl LlmHttpPlanner {
    pub fn new(base_url: &str) -> Self {
        Self {
            url: format!("{}/v1/chat/completions", base_url.trim_end_matches('/')),
        }
    }
}

#[derive(serde::Deserialize)]
struct ChatResponse {
    #[serde(default)]
    choices: Vec<Choice>,
    #[serde(default)]
    usage: Option<Usage>,
}
#[derive(serde::Deserialize)]
struct Choice {
    message: Message,
}
#[derive(serde::Deserialize, Clone)]
struct Message {
    #[serde(default)]
    content: Option<String>,
    #[serde(default, rename = "reasoning_content")]
    reasoning: Option<String>,
}
#[derive(serde::Deserialize)]
struct Usage {
    #[serde(default)]
    completion_tokens: u64,
}

#[derive(serde::Deserialize)]
struct RawNode {
    #[serde(default)]
    node_id: Option<String>,
    #[serde(default)]
    dependencies: Vec<String>,
    #[serde(default)]
    required_capabilities: Vec<String>,
    #[serde(default)]
    prompt: Option<String>,
}

async fn chat(url: &str, prompt: &str) -> Result<(u64, Message), String> {
    let client = reqwest::Client::builder()
        .timeout(std::time::Duration::from_secs(240))
        .build()
        .map_err(|e| format!("client: {e}"))?;
    let body = serde_json::json!({
        "messages": [
            {"role": "system", "content": PLANNER_SYSTEM},
            {"role": "user", "content": prompt},
        ],
        "max_tokens": 2048,
        "temperature": 0.0,
    });
    let resp: ChatResponse = client
        .post(url)
        .json(&body)
        .send()
        .await
        .map_err(|e| format!("http: {e}"))?
        .error_for_status()
        .map_err(|e| format!("status: {e}"))?
        .json()
        .await
        .map_err(|e| format!("json: {e}"))?;
    let usage = resp.usage.map(|u| u.completion_tokens).unwrap_or(0);
    let msg = resp
        .choices
        .into_iter()
        .next()
        .map(|c| c.message)
        .ok_or_else(|| "sem choices".to_string())?;
    Ok((usage, msg))
}

impl Planner for LlmHttpPlanner {
    fn plan(&self, input: &crate::engine::PlanRequest) -> Result<PlanCandidate, String> {
        let url = self.url.clone();
        let prompt = input.prompt.clone();
        let (usage, msg) = std::thread::spawn(move || {
            let rt = tokio::runtime::Builder::new_current_thread()
                .enable_all()
                .build()
                .map_err(|e| format!("runtime: {e}"))?;
            rt.block_on(chat(&url, &prompt))
        })
        .join()
        .map_err(|_| "thread do planner panicou".to_string())??;

        let content = msg.content.as_deref().unwrap_or("");
        let raw = extract_json_array(content)
            .or_else(|| extract_json_array(msg.reasoning.as_deref().unwrap_or("")))
            .ok_or_else(|| "JSON de subtasks não encontrado".to_string())?;
        let parsed: Vec<RawNode> = serde_json::from_str(&raw).map_err(|e| format!("parse: {e}"))?;
        if parsed.is_empty() {
            return Err("planner não produziu nós".into());
        }
        let nodes = parsed
            .into_iter()
            .enumerate()
            .take(4)
            .map(|(i, n)| CandidateNode {
                node_id: n.node_id.unwrap_or_else(|| format!("n{}", i + 1)),
                kind: NodeKindCand::Inference,
                required_capabilities: n.required_capabilities,
                model_id: String::new(),
                target_agent_id: String::new(),
                prompt_template: n.prompt.unwrap_or_default(),
                dependencies: n.dependencies,
                output_names: vec!["out".into()],
                inputs: vec![],
                max_attempts: 1,
                max_tokens: 256,
                max_output_bytes: 65_536,
                deadline_ns: 0,
                priority: 5,
                side_effect_key: String::new(),
            })
            .collect();
        Ok(PlanCandidate {
            planner_id: "llm-gemma4-12b".into(),
            planner_revision: "b11671+UD-Q4_K_XL".into(),
            planner_tokens: usage,
            nodes,
        })
    }
}

/// Extrai o primeiro array JSON "[...]" bem-formado do texto (byte-scan com
/// contagem de colchetes e strings escapadas — sem regex com backtracking).
pub fn extract_json_array(text: &str) -> Option<String> {
    let bytes = text.as_bytes();
    let mut depth = 0i32;
    let mut start = None;
    let mut in_string = false;
    let mut escape = false;
    for (i, &b) in bytes.iter().enumerate() {
        if in_string {
            if escape {
                escape = false;
            } else if b == b'\\' {
                escape = true;
            } else if b == b'"' {
                in_string = false;
            }
            continue;
        }
        match b {
            b'"' => in_string = true,
            b'[' => {
                if depth == 0 {
                    start = Some(i);
                }
                depth += 1;
            }
            b']' => {
                depth -= 1;
                if depth == 0 {
                    if let Some(s) = start {
                        let candidate = &text[s..=i];
                        if serde_json::from_str::<serde_json::Value>(candidate).is_ok() {
                            return Some(candidate.to_string());
                        }
                        start = None;
                    }
                }
            }
            _ => {}
        }
    }
    None
}

#[cfg(test)]
mod tests {
    use super::*;

    #[test]
    fn extracts_first_valid_array_skipping_nested_and_strings() {
        let text = r#"blabla [não-json] depois ```json
[{"node_id":"n1","prompt":"use [brackets] \"quoted\""},{"node_id":"n2"}]
``` fim"#;
        let got = extract_json_array(text).expect("array");
        let v: serde_json::Value = serde_json::from_str(&got).unwrap();
        assert_eq!(v.as_array().unwrap().len(), 2);
    }

    #[test]
    fn no_array_returns_none() {
        assert!(extract_json_array("sem nada aqui").is_none());
    }
}
