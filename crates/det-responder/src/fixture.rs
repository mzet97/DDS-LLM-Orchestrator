//! Derivação determinística de fixtures a partir da requisição real.
//!
//! Réplica exata das semânticas de `bench/backend_deterministic/server.py`:
//! normalização `role:conteúdo` unida por `\n`, `sha256` hex `[..16]` e o
//! texto `[fixture stage={S} h={h} n={n}]`. A etapa vem de correspondência
//! explícita com o marcador `PROMPT_VERSION:` da mensagem `system` — nunca
//! pela ordem de chegada.

use sha2::{Digest, Sha256};

/// Correspondência explícita marcador → (etapa, família). Derivada dos
/// `PROMPT_VERSION:` dos arquivos em `benchmarks/orchestration/prompts/`.
const STAGE_TABLE: &[(&str, &str, &str)] = &[
    ("seq_analyst_v1", "A", "seq"),
    ("seq_reviewer_v1", "B", "seq"),
    ("seq_consolidator_v1", "C", "seq"),
    ("fork_correctness_v1", "A", "fork"),
    ("fork_security_v1", "B", "fork"),
    ("fork_consolidator_v1", "C", "fork"),
];

/// Erros de validação da requisição (viram `LLMInferenceError` observável).
#[derive(Debug, Clone, Copy, PartialEq, Eq)]
pub enum Invalid {
    EmptyMessages,
    MissingText,
    UnknownStage,
    EmptyModel,
}

impl Invalid {
    pub const fn message(self) -> &'static str {
        match self {
            Self::EmptyMessages => "messages_json deve ser array não vazio",
            Self::MissingText => "mensagem sem role/content textual",
            Self::UnknownStage => "etapa desconhecida: sem marcador PROMPT_VERSION",
            Self::EmptyModel => "model_name vazio",
        }
    }
}

fn msg_content(v: &serde_json::Value) -> Option<String> {
    match v.get("content") {
        Some(serde_json::Value::String(s)) => Some(s.clone()),
        // Espelha o backend Python: conteúdo em lista de partes {text}.
        Some(serde_json::Value::Array(parts)) => {
            let mut out = Vec::new();
            for p in parts {
                if let Some(t) = p.get("text").and_then(|t| t.as_str()) {
                    out.push(t);
                }
            }
            Some(out.join(" "))
        }
        _ => None,
    }
}

/// Réplica exata de `normalize` do backend Python.
fn normalize(messages: &[serde_json::Value]) -> Option<String> {
    let mut parts = Vec::with_capacity(messages.len());
    for m in messages {
        let role = m.get("role").and_then(|r| r.as_str()).unwrap_or("");
        parts.push(format!("{role}:{}", msg_content(m)?));
    }
    Some(parts.join("\n"))
}

pub fn content_hash(normalized: &str) -> String {
    format!("{:x}", Sha256::digest(normalized.as_bytes()))[..16].to_string()
}

/// Requisição parseada e validada, com etapa derivada por correspondência.
#[derive(Debug)]
pub struct Parsed {
    pub messages: Vec<serde_json::Value>,
    pub normalized: String,
    pub stage: &'static str,
    pub family: &'static str,
}

pub fn parse(messages_json: &str) -> Result<Parsed, Invalid> {
    let messages: Vec<serde_json::Value> = match serde_json::from_str(messages_json) {
        Ok(serde_json::Value::Array(v)) if !v.is_empty() => v,
        _ => return Err(Invalid::EmptyMessages),
    };
    let normalized = normalize(&messages).ok_or(Invalid::MissingText)?;
    let system_text: String = messages
        .iter()
        .filter(|m| m.get("role").and_then(|r| r.as_str()) == Some("system"))
        .filter_map(msg_content)
        .collect::<Vec<_>>()
        .join("\n");
    for (marker, stage, family) in STAGE_TABLE {
        if system_text.contains(&format!("PROMPT_VERSION: {marker}")) {
            return Ok(Parsed {
                messages,
                normalized,
                stage,
                family,
            });
        }
    }
    Err(Invalid::UnknownStage)
}

pub fn fixture_text(stage: &str, hash: &str, n: usize) -> String {
    format!("[fixture stage={stage} h={hash} n={n}]")
}

/// Ponto de corte no meio do texto sem quebrar caractere UTF-8.
pub fn split_midpoint(s: &str) -> usize {
    let mut mid = s.len() / 2;
    while !s.is_char_boundary(mid) {
        mid += 1;
    }
    mid
}

#[cfg(test)]
mod tests {
    use super::{content_hash, fixture_text, parse, split_midpoint, Invalid};

    fn msgs(sys: &str, extra: &[(&str, &str)]) -> String {
        let mut v = vec![serde_json::json!({"role": "system", "content": sys})];
        for (role, content) in extra {
            v.push(serde_json::json!({"role": role, "content": content}));
        }
        serde_json::Value::Array(v).to_string()
    }

    #[test]
    fn stage_a_seq_por_marcador_explicito() {
        let p = parse(&msgs(
            "PROMPT_VERSION: seq_analyst_v1\nSYSTEM: x",
            &[("user", "ENTRADA: e")],
        ))
        .expect("parse");
        assert_eq!((p.stage, p.family), ("A", "seq"));
        assert_eq!(p.messages.len(), 2);
    }

    #[test]
    fn ordem_de_chegada_nao_importa_para_etapa() {
        // B antes de A na mesma carga: cada um deriva sua etapa sozinho.
        let b = parse(&msgs("PROMPT_VERSION: seq_reviewer_v1", &[("user", "x")])).expect("parse");
        let a = parse(&msgs("PROMPT_VERSION: seq_analyst_v1", &[("user", "x")])).expect("parse");
        assert_eq!((b.stage, a.stage), ("B", "A"));
    }

    #[test]
    fn marcador_desconhecido_e_erro_nao_fixture() {
        assert_eq!(
            parse(&msgs("sem marcador", &[("user", "x")])).expect_err("deveria falhar"),
            Invalid::UnknownStage
        );
    }

    #[test]
    fn mensagens_vazias_e_erro() {
        assert_eq!(
            parse("[]").expect_err("deveria falhar"),
            Invalid::EmptyMessages
        );
    }

    #[test]
    fn hash_igual_ao_backend_python() {
        // normalize("system:PROMPT_VERSION: seq_analyst_v1\nuser:ENTRADA: e"):
        // valor de referência gerado pelo backend Python (sha256[:16]).
        let norm = "system:PROMPT_VERSION: seq_analyst_v1\nuser:ENTRADA: e";
        std::fs::write("/tmp/fx-norm.txt", norm).expect("write");
        let out = std::process::Command::new("python3")
            .args(["-c", "import hashlib;print(hashlib.sha256(open('/tmp/fx-norm.txt','rb').read()).hexdigest()[:16])"])
            .output()
            .expect("python3");
        assert!(out.status.success());
        let expected = String::from_utf8(out.stdout)
            .expect("utf8")
            .trim()
            .to_string();
        assert_eq!(content_hash(norm), expected);
    }

    #[test]
    fn fixture_formato_compativel() {
        assert_eq!(
            fixture_text("B", "0123456789abcdef", 3),
            "[fixture stage=B h=0123456789abcdef n=3]"
        );
    }

    #[test]
    fn split_nao_quebra_utf8() {
        let s = "ação [fixture]";
        let mid = split_midpoint(s);
        assert!(s.is_char_boundary(mid));
        assert_eq!(format!("{}{}", &s[..mid], &s[mid..]), s);
    }
}
