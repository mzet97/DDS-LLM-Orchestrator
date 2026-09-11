//! Montagem canônica de mensagens por etapa — réplica exata de
//! `benchmarks/orchestration/src/bench/systems/assembly.py`:
//! `[system, "ENTRADA: {entry}", "{LABEL}: {conteúdo}"...]`, ordem preservada.

pub const ANALYSIS: &str = "analysis";
pub const REVIEW: &str = "review";
pub const CORRECTNESS: &str = "correctness";
pub const SECURITY: &str = "security";

pub fn load_prompt(dir: &str, name: &str) -> Result<String, std::io::Error> {
    std::fs::read_to_string(format!("{dir}/{name}"))
}

pub fn build_messages(system: &str, entry: &str, prior: &[(&str, String)]) -> String {
    let mut msgs = vec![
        serde_json::json!({"role": "system", "content": system}),
        serde_json::json!({"role": "user", "content": format!("ENTRADA: {entry}")}),
    ];
    for (label, content) in prior {
        msgs.push(serde_json::json!({"role": "user", "content": format!("{label}: {content}")}));
    }
    serde_json::Value::Array(msgs).to_string()
}

#[cfg(test)]
mod tests {
    use super::{build_messages, ANALYSIS};

    #[test]
    fn montagem_igual_ao_python() {
        // Réplica byte-a-byte da regra: valores parseados devem coincidir com
        // assembly.build_messages("SYS", "E", (("analysis", "A"),)).
        let got: serde_json::Value =
            serde_json::from_str(&build_messages("SYS", "E", &[(ANALYSIS, "A".to_string())]))
                .expect("json");
        assert_eq!(
            got,
            serde_json::json!([
                {"role": "system", "content": "SYS"},
                {"role": "user", "content": "ENTRADA: E"},
                {"role": "user", "content": "analysis: A"},
            ])
        );
    }
}
