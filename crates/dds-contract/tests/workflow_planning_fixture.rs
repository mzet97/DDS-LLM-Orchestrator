//! T-900-M1-01 — gate durável do contrato WorkflowPlanning (fase 900).
//!
//! Roda SEM a feature `dds` (leitura de arquivos + JSON + SHA-256) e existe
//! ANTES da integração do IDL no `build.rs` (essa é a T-900-M1-02). Congela:
//!
//! 1. IDL ↔ fixture `idl/WorkflowPlanning.schema.json`: ordem de campos,
//!    chaves, bounds de string/sequence e literais/ordinais de enum.
//! 2. Os cinco tópicos/typenames/chaves da proposta M0-1 (contracts.md).
//! 3. Vetores golden do encoding de digest (`payload_hash`/`plan_hash`) em
//!    `idl/WorkflowPlanning.digest_vectors.json`: os hashes foram produzidos
//!    pela implementação de referência em Python
//!    (`specs/900-laya-adaptive-routing/tools/m1_01_fixture_tool.py`) e este
//!    teste os reproduz em Rust — prova cross-language das fixtures.
//!
//! O encoding de digest é a preimage documental (contracts.md §Compatibilidade),
//! NÃO o CDR do wire (XCDR1 é gate de interop M1-04/V02).

use serde_json::Value;

fn manifest(relative: &str) -> String {
    let path = std::path::Path::new(env!("CARGO_MANIFEST_DIR")).join(relative);
    std::fs::read_to_string(&path).unwrap_or_else(|e| panic!("ler {}: {e}", path.display()))
}

fn schema() -> Value {
    serde_json::from_str(&manifest("idl/WorkflowPlanning.schema.json")).expect("schema fixture")
}

fn vectors() -> Value {
    serde_json::from_str(&manifest("idl/WorkflowPlanning.digest_vectors.json"))
        .expect("digest vectors fixture")
}

fn strip_comments(src: &str) -> String {
    src.lines()
        .map(|l| match l.find("//") {
            Some(i) => &l[..i],
            None => l,
        })
        .collect::<Vec<_>>()
        .join("\n")
}

/// Ordem declarada de literais por enum, lida do IDL.
fn idl_enums(idl: &str) -> Vec<(String, Vec<String>)> {
    let re = regex::Regex::new(r"enum\s+(\w+)\s*\{([^}]*)\}").unwrap();
    re.captures_iter(idl)
        .map(|c| {
            let literals = c[2]
                .split(',')
                .map(|l| l.trim())
                .filter(|l| !l.is_empty())
                .map(str::to_string)
                .collect();
            (c[1].to_string(), literals)
        })
        .collect()
}

/// Campo parseado do IDL: (nome, declaração de tipo, é_chave).
type IdlField = (String, String, bool);

/// Campos por struct no IDL, na ordem declarada.
fn idl_struct_fields(idl: &str) -> Vec<(String, Vec<IdlField>)> {
    let re = regex::Regex::new(r"(?s)struct\s+(\w+)\s*\{(.*?)\};").unwrap();
    re.captures_iter(idl)
        .map(|c| {
            let name = c[1].to_string();
            let mut fields = Vec::new();
            let mut key_pending = false;
            for raw in c[2].lines() {
                let line = raw.trim();
                if line.is_empty() {
                    continue;
                }
                let (line, key_here) = match line.strip_prefix("@key") {
                    Some(rest) => (rest.trim(), true),
                    None => (line, false),
                };
                if line.starts_with('@') || line.is_empty() {
                    if key_here {
                        key_pending = true;
                    }
                    continue;
                }
                let key = key_pending || key_here;
                key_pending = false;
                let mut parts = line
                    .trim_end_matches(';')
                    .trim()
                    .rsplitn(2, char::is_whitespace);
                let fname = parts.next().unwrap().trim().to_string();
                let tdecl = parts.next().unwrap_or("").trim().to_string();
                fields.push((fname, tdecl, key));
            }
            (name, fields)
        })
        .collect()
}

/// Tipo IDL canônico esperado para um campo da fixture.
fn idl_type_of(f: &Value) -> String {
    let t = f["type"].as_str().unwrap();
    match t {
        "string" => match f.get("bound") {
            Some(b) => format!("string<{}>", b.as_u64().unwrap()),
            None => "string".to_string(),
        },
        "seq" => {
            let elem = idl_type_of(&f["elem"]);
            match f.get("bound") {
                Some(b) => format!("sequence<{elem}, {}>", b.as_u64().unwrap()),
                None => format!("sequence<{elem}>"),
            }
        }
        "enum" => f["enum"].as_str().unwrap().to_string(),
        "struct" => f["struct"].as_str().unwrap().to_string(),
        "u16" => "unsigned short".to_string(),
        "u32" => "unsigned long".to_string(),
        "u64" => "unsigned long long".to_string(),
        "i32" => "long".to_string(),
        "f64" => "double".to_string(),
        "bool" => "boolean".to_string(),
        other => panic!("tipo da fixture desconhecido: {other}"),
    }
}

#[test]
fn schema_fixture_matches_idl_source() {
    let idl = strip_comments(&manifest("idl/WorkflowPlanning.idl"));
    let schema = schema();

    // módulo
    let module_re = regex::Regex::new(r"module\s+(\w+)\s*\{").unwrap();
    let module = module_re.captures(&idl).expect("module no IDL")[1].to_string();
    assert_eq!(module, schema["module"].as_str().unwrap());
    assert_eq!(module, "dds_llm_workflow_v1");

    // enums: mesmos nomes, mesmos literais na mesma ordem (ordinais implícitos)
    let idl_enums = idl_enums(&idl);
    let fixture_enums = schema["enums"].as_object().unwrap();
    assert_eq!(
        idl_enums.len(),
        fixture_enums.len(),
        "quantidade de enums divergiu entre IDL e fixture"
    );
    for (name, literals) in &idl_enums {
        let want = fixture_enums[name]["literals"]
            .as_array()
            .unwrap()
            .iter()
            .map(|l| l.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(literals, &want, "literais/ordem do enum {name}");
    }

    // structs: mesmos campos, mesma ordem, mesmos tipos (incl. bounds), mesmas chaves
    let idl_structs = idl_struct_fields(&idl);
    let fixture_structs = schema["structs"].as_object().unwrap();
    assert_eq!(
        idl_structs.len(),
        fixture_structs.len(),
        "quantidade de structs divergiu entre IDL e fixture"
    );
    for (name, fields) in &idl_structs {
        let fspec = &fixture_structs[name]["fields"];
        let fspec = fspec.as_array().unwrap();
        assert_eq!(
            fields.len(),
            fspec.len(),
            "quantidade de campos de {name} divergiu (IDL {} vs fixture {})",
            fields.len(),
            fspec.len()
        );
        for ((fname, tdecl, is_key), fspec) in fields.iter().zip(fspec) {
            assert_eq!(
                fname,
                fspec["name"].as_str().unwrap(),
                "ordem/nome em {name}"
            );
            assert_eq!(
                tdecl,
                &idl_type_of(fspec),
                "tipo do campo {name}.{fname} (IDL `{tdecl}` vs fixture `{}`)",
                idl_type_of(fspec)
            );
            assert_eq!(
                *is_key,
                fspec.get("key").and_then(|k| k.as_bool()).unwrap_or(false),
                "flag @key de {name}.{fname}"
            );
        }
    }
}

#[test]
fn five_topics_freeze_the_m0_1_contract() {
    let schema = schema();
    let expected: [(&str, &str, &[&str]); 5] = [
        (
            "Workflow.Intent",
            "dds_llm_workflow_v1::WorkflowIntent",
            &["request_id"],
        ),
        (
            "Routing.Decision",
            "dds_llm_workflow_v1::RoutingDecision",
            &["request_id", "decision_id"],
        ),
        (
            "Agent.Capabilities",
            "dds_llm_workflow_v1::AgentCapabilities",
            &["agent_id"],
        ),
        (
            "Workflow.Plan",
            "dds_llm_workflow_v1::WorkflowPlan",
            &["workflow_id", "plan_revision"],
        ),
        (
            "Workflow.State",
            "dds_llm_workflow_v1::WorkflowState",
            &["workflow_id", "plan_revision"],
        ),
    ];
    let topics = schema["topics"].as_array().unwrap();
    assert_eq!(topics.len(), 5);
    for (spec, topic) in expected.iter().zip(topics) {
        assert_eq!(topic["topic"].as_str().unwrap(), spec.0);
        assert_eq!(topic["typename"].as_str().unwrap(), spec.1);
        let keys = topic["keys"]
            .as_array()
            .unwrap()
            .iter()
            .map(|k| k.as_str().unwrap())
            .collect::<Vec<_>>();
        assert_eq!(keys, spec.2, "chaves do tópico {}", spec.0);
        // typename aponta para struct presente na fixture
        let struct_name = topic["struct"].as_str().unwrap();
        assert!(schema["structs"][struct_name].is_object());
    }

    // Regra de versão: os cinco tipos de tópico carregam schema_version (u16),
    // com valor obrigatório 1 em V1 garantido por validação (IDL 4.2 não tem
    // default de membro — ambos os geradores rejeitam `= 1`; ver cabeçalho do
    // IDL e evidence/m1-01).
    for topic in topics {
        let struct_name = topic["struct"].as_str().unwrap();
        let fields = schema["structs"][struct_name]["fields"].as_array().unwrap();
        let sv = fields
            .iter()
            .find(|f| f["name"].as_str() == Some("schema_version"))
            .unwrap_or_else(|| panic!("{struct_name} sem schema_version"));
        assert_eq!(sv["type"].as_str().unwrap(), "u16");
    }
}

// ---------------------------------------------------------------- digest

fn enc_typed(tv: &Value, schema: &Value, out: &mut Vec<u8>) {
    let t = tv["t"].as_str().unwrap();
    let v = &tv["v"];
    match t {
        "string" => {
            let b = v.as_str().unwrap().as_bytes();
            out.extend_from_slice(&(b.len() as u32).to_be_bytes());
            out.extend_from_slice(b);
        }
        "u16" => out.extend_from_slice(&(v.as_u64().unwrap() as u16).to_be_bytes()),
        "u32" => out.extend_from_slice(&(v.as_u64().unwrap() as u32).to_be_bytes()),
        "u64" => out.extend_from_slice(&v.as_u64().unwrap().to_be_bytes()),
        "i32" => out.extend_from_slice(&(v.as_i64().unwrap() as i32).to_be_bytes()),
        "bool" => out.push(u8::from(v.as_bool().unwrap())),
        "f64" => {
            let f = v.as_f64().unwrap();
            assert!(f.is_finite(), "NaN/infinito é rejeitado no digest");
            let f = if f == 0.0 { 0.0 } else { f }; // normaliza -0.0 -> +0.0
            out.extend_from_slice(&f.to_be_bytes());
        }
        "enum" => {
            let literals = schema["enums"][tv["enum"].as_str().unwrap()]["literals"]
                .as_array()
                .unwrap();
            let idx = literals
                .iter()
                .position(|l| l.as_str() == v.as_str())
                .unwrap_or_else(|| panic!("literal {} fora do enum", v.as_str().unwrap()));
            out.extend_from_slice(&(idx as u32).to_be_bytes());
        }
        "seq" => {
            let items = v.as_array().unwrap();
            out.extend_from_slice(&(items.len() as u32).to_be_bytes());
            for item in items {
                enc_typed(item, schema, out);
            }
        }
        "struct" => {
            let fields = schema["structs"][tv["struct"].as_str().unwrap()]["fields"]
                .as_array()
                .unwrap();
            for f in fields {
                enc_typed(&v[f["name"].as_str().unwrap()], schema, out);
            }
        }
        other => panic!("tipo tipado desconhecido: {other}"),
    }
}

fn hex(bytes: &[u8]) -> String {
    bytes.iter().map(|b| format!("{b:02x}")).collect()
}

#[test]
fn digest_vectors_reproduce_in_rust() {
    use sha2::{Digest, Sha256};

    let schema = schema();
    let all = vectors();
    let vectors = all["vectors"].as_array().unwrap();
    assert_eq!(
        vectors.len(),
        16,
        "conjunto completo de vetores golden (15 + decision-full desde M2-01)"
    );

    for vec in vectors {
        let id = vec["id"].as_str().unwrap();
        let skip = vec["skip_field"].as_str().unwrap_or("");
        let mut out = Vec::new();
        if vec["value"]["t"].as_str().unwrap() == "struct" {
            let fields = schema["structs"][vec["value"]["struct"].as_str().unwrap()]["fields"]
                .as_array()
                .unwrap();
            for f in fields {
                if !skip.is_empty() && f["name"].as_str().unwrap() == skip {
                    continue; // preimage exclui o próprio campo hash
                }
                enc_typed(
                    &vec["value"]["v"][f["name"].as_str().unwrap()],
                    &schema,
                    &mut out,
                );
            }
        } else {
            enc_typed(&vec["value"], &schema, &mut out);
        }
        let expected_pre = vec["preimage_hex"].as_str().unwrap();
        assert_eq!(hex(&out), expected_pre, "preimage do vetor {id}");
        let sha = Sha256::digest(&out);
        let expected_sha = vec["sha256"].as_str().unwrap();
        assert_eq!(
            hex(&sha),
            expected_sha,
            "sha256 do vetor {id} (cross-language Python==Rust)"
        );
    }
}

/// Chaves de nível-topo de um JSON serializado, em ordem de declaração.
/// Amostras usam Default (valores vazios/zero), então toda `"chave":` após
/// `{` ou `,` no depth 1 é campo do struct — valores (`:""`) são ignorados.
#[cfg(not(feature = "dds"))]
fn top_level_json_keys(json: &str) -> Vec<String> {
    let mut keys = Vec::new();
    let mut depth = 0usize;
    let chars: Vec<char> = json.chars().collect();
    let mut i = 0;
    while i < chars.len() {
        match chars[i] {
            '{' => depth += 1,
            '}' => depth = depth.saturating_sub(1),
            '"' if depth == 1 => {
                let prev = if i > 0 { chars[i - 1] } else { '{' };
                if prev == '{' || prev == ',' {
                    let mut key = String::new();
                    i += 1;
                    while i < chars.len() && chars[i] != '"' {
                        key.push(chars[i]);
                        i += 1;
                    }
                    keys.push(key);
                }
            }
            _ => {}
        }
        i += 1;
    }
    keys
}

/// T-900-M1-02: gate mock↔IDL. Os mocks (`workflow::dds_llm_workflow_v1`,
/// build sem `dds`) precisam ter exatamente os campos do IDL na mesma ordem.
/// Geração por `specs/900-laya-adaptive-routing/tools/m1_02_mock_gen.py`;
/// este teste trava drift manual ou de regeneração. Gate `not(dds)` como o
/// `mock_parity.rs` (com feature unificada, `workflow::*` vira o tipo gerado).
#[cfg(not(feature = "dds"))]
#[test]
fn mock_workflow_types_match_idl_fixture() {
    use dds_contract::workflow::dds_llm_workflow_v1 as m;

    let schema = schema();
    let cases: Vec<(&str, String)> = vec![
        (
            "ProfileProbability",
            serde_json::to_string(&m::ProfileProbability::default()).unwrap(),
        ),
        (
            "CapabilityProbability",
            serde_json::to_string(&m::CapabilityProbability::default()).unwrap(),
        ),
        (
            "ToolVersion",
            serde_json::to_string(&m::ToolVersion::default()).unwrap(),
        ),
        (
            "InputRef",
            serde_json::to_string(&m::InputRef::default()).unwrap(),
        ),
        (
            "OutputRef",
            serde_json::to_string(&m::OutputRef::default()).unwrap(),
        ),
        (
            "PlanNode",
            serde_json::to_string(&m::PlanNode::default()).unwrap(),
        ),
        (
            "NodeState",
            serde_json::to_string(&m::NodeState::default()).unwrap(),
        ),
        (
            "WorkflowIntent",
            serde_json::to_string(&m::WorkflowIntent::default()).unwrap(),
        ),
        (
            "RoutingDecision",
            serde_json::to_string(&m::RoutingDecision::default()).unwrap(),
        ),
        (
            "AgentCapabilities",
            serde_json::to_string(&m::AgentCapabilities::default()).unwrap(),
        ),
        (
            "WorkflowPlan",
            serde_json::to_string(&m::WorkflowPlan::default()).unwrap(),
        ),
        (
            "WorkflowState",
            serde_json::to_string(&m::WorkflowState::default()).unwrap(),
        ),
    ];
    assert_eq!(cases.len(), schema["structs"].as_object().unwrap().len());

    for (name, json) in cases {
        let want: Vec<&str> = schema["structs"][name]["fields"]
            .as_array()
            .unwrap()
            .iter()
            .map(|f| f["name"].as_str().unwrap())
            .collect();
        let got = top_level_json_keys(&json);
        assert_eq!(
            got, want,
            "mock {name} divergiu da fixture do IDL (campos/ordem)"
        );
    }

    // Ordinais canônicos dos enums (emenda 3: campos carregam i32).
    assert_eq!(m::IntentCommand::IC_SUBMIT as i32, 0);
    assert_eq!(m::IntentCommand::IC_CANCEL as i32, 1);
    assert_eq!(m::QosIntent::QI_FAILOVER as i32, 4);
    assert_eq!(m::DecisionStatus::DS_ERROR as i32, 1);
    assert_eq!(m::ReasonCode::RC_CANCELLED as i32, 14);
    assert_eq!(m::WorkflowStatus::WS_IN_DOUBT as i32, 7);
    assert_eq!(m::NodeStatus::NS_IN_DOUBT as i32, 8);
    assert_eq!(m::NodeKind::NK_TOOL as i32, 1);
}
