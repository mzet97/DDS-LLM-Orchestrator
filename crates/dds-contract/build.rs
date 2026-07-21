//! Gera tipos Rust a partir dos IDLs canônicos (REQ-001).
//!
//! - `OrchestratorDDS.idl` — tipos LLM + ServerStatus (interop C++/Python)
//! - `OrchestratorV4.idl` — Task/AgentState/TaskOutput/SystemMetric
//!
//! O V4 usa `#pragma keylist`, que o parser built-in do cyclonedds-build não
//! aceita; sanitizamos para anotações `@key` antes de compilar.
//!
//! Após a geração, reescrevemos `type_name()` para o typename qualificado
//! (`module::Struct`) que o C++/Python usam no XTypes matching.

use regex::Regex;
use std::collections::HashMap;
use std::env;
use std::fs;
use std::path::{Path, PathBuf};

fn main() {
    let dds_enabled = env::var("CARGO_FEATURE_DDS").is_ok();
    println!("cargo:rerun-if-changed=build.rs");

    let manifest_dir = PathBuf::from(env::var("CARGO_MANIFEST_DIR").unwrap());
    // crates/dds-contract -> src/rust -> src -> tese -> third_party/llama.cpp_dds/...
    //
    // Repontado em 2026-07-20: `tese/src/llama_cpp/` está sendo descontinuado em favor
    // de `tese/third_party/llama.cpp_dds/` (fonte atual da integração C++/DDS). O IDL
    // V4 de `third_party/llama.cpp_dds` estava desatualizado (pré-WF-3, só 4 tipos,
    // `#pragma keylist`) — foi re-sincronizado byte-a-byte com a versão de
    // `src/llama_cpp` (14 tipos, `@key`, já validada contra SEDP do Python) antes deste
    // repontamento. Ver OPTIMIZATION_AUDIT.md para o histórico da divergência.
    let idl_dds = resolve(
        &manifest_dir,
        "../../../../third_party/llama.cpp_dds/dds/idl/OrchestratorDDS.idl",
    );
    let idl_v4 = resolve(
        &manifest_dir,
        "../../../../third_party/llama.cpp_dds/dds/v4/idl/OrchestratorV4.idl",
    );

    println!("cargo:rerun-if-changed={}", idl_dds.display());
    println!("cargo:rerun-if-changed={}", idl_v4.display());
    println!(
        "cargo:rerun-if-changed={}",
        idl_dds.with_extension("c").display()
    );
    println!(
        "cargo:rerun-if-changed={}",
        idl_v4.with_extension("c").display()
    );

    if !dds_enabled {
        return;
    }

    let out_dir = PathBuf::from(env::var("OUT_DIR").unwrap());

    compile_idl(&idl_dds, &out_dir, "OrchestratorDDS", false);
    compile_idl(&idl_v4, &out_dir, "OrchestratorV4", true);

    for (file, module, types, c_file) in [
        (
            "OrchestratorDDS.rs",
            "orchestrator",
            &[
                "LLMInferenceRequest",
                "LLMInferenceResult",
                "LLMInferenceError",
                "ServerStatus",
            ][..],
            idl_dds.with_extension("c"),
        ),
        (
            "OrchestratorV4.rs",
            "dds_llm_orchestrator",
            &[
                "Task",
                "AgentState",
                "TaskOutput",
                "SystemMetric",
                "QoSRoutingProfile",
                "ContextSnapshot",
                "ContextUpdate",
                "ToolCallRequest",
                "ExecutionTraceEvent",
                "SecurityPolicySnapshot",
                "SecurityPolicyUpdate",
                "QoSMetric",
                "QoSViolation",
                "DiscoveryEvent",
            ][..],
            idl_v4.with_extension("c"),
        ),
    ] {
        let path = out_dir.join(file);
        strip_inner_allow_attrs(&path);
        qualify_type_names(&path, module, types);
        inject_type_metadata(&path, module, types, &c_file);
    }
}

/// `include!` não aceita `#![…]` no arquivo incluído; troca por outer attrs.
fn strip_inner_allow_attrs(path: &Path) {
    if !path.exists() {
        return;
    }
    let src = fs::read_to_string(path).expect("read generated");
    let fixed = src.replace(
        "#![allow(unused_imports, dead_code, non_camel_case_types, non_snake_case)]",
        "#[allow(unused_imports, dead_code, non_camel_case_types, non_snake_case)]",
    );
    fs::write(path, fixed).expect("write stripped allow");
}

fn resolve(base: &Path, rel: &str) -> PathBuf {
    let p = base.join(rel);
    p.canonicalize().unwrap_or(p)
}

fn compile_idl(idl_path: &Path, out_dir: &Path, module_name: &str, sanitize: bool) {
    let options = cyclonedds_build::CompileOptions {
        cyclonedds_home: None,
        output_dir: Some(out_dir.to_path_buf()),
        try_idlc: false,
        module_name: Some(module_name.to_string()),
    };

    if sanitize {
        let raw = fs::read_to_string(idl_path)
            .unwrap_or_else(|e| panic!("read {}: {e}", idl_path.display()));
        let sanitized = sanitize_pragma_keylist(&raw);
        let tmp = out_dir.join(format!("{module_name}_sanitized.idl"));
        fs::write(&tmp, sanitized).expect("write sanitized idl");
        cyclonedds_build::compile_idl_with_options(&tmp, &options)
            .unwrap_or_else(|e| panic!("compile sanitized {module_name}: {e}"));
    } else {
        cyclonedds_build::compile_idl_with_options(idl_path, &options)
            .unwrap_or_else(|e| panic!("compile {module_name}: {e}"));
    }
}

/// Converte `#pragma keylist Struct f1 f2` em anotações `@key` nos campos.
fn sanitize_pragma_keylist(src: &str) -> String {
    let re_pragma = Regex::new(r"(?m)^\s*#pragma\s+keylist\s+(\w+)\s+([^\n;]+)").unwrap();
    let mut keylists: HashMap<String, Vec<String>> = HashMap::new();
    for cap in re_pragma.captures_iter(src) {
        let name = cap[1].to_string();
        let keys: Vec<String> = cap[2]
            .split_whitespace()
            .map(|s| s.trim_matches(';').to_string())
            .filter(|s| !s.is_empty())
            .collect();
        keylists.insert(name, keys);
    }

    let mut out = re_pragma.replace_all(src, "").into_owned();
    // Strip remaining preprocessor-style `#...` lines (no lookaround; regex crate
    // default engine doesn't support it). Skip pure IDL that starts with `@`.
    let mut cleaned = String::with_capacity(out.len());
    for line in out.lines() {
        let trimmed = line.trim_start();
        if trimmed.starts_with('#') {
            let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
            cleaned.push_str(&indent);
            cleaned.push_str("// stripped hash comment\n");
        } else {
            cleaned.push_str(line);
            cleaned.push('\n');
        }
    }
    out = cleaned;

    for (struct_name, keys) in &keylists {
        out = inject_keys(&out, struct_name, keys);
    }
    out
}

fn inject_keys(text: &str, struct_name: &str, keys: &[String]) -> String {
    let re = Regex::new(&format!(
        r"(struct\s+{struct_name}\s*\{{)(.*?)(\n\s*\}};\s*)"
    ))
    .unwrap();
    re.replace_all(text, |caps: &regex::Captures| {
        let header = &caps[1];
        let body = &caps[2];
        let tail = &caps[3];
        let mut new_lines = Vec::new();
        for line in body.lines() {
            let stripped = line.trim();
            if let Some(field) = field_name_from_decl(stripped) {
                if keys.iter().any(|k| k == field) {
                    let indent: String = line.chars().take_while(|c| c.is_whitespace()).collect();
                    new_lines.push(format!("{indent}@key"));
                }
            }
            new_lines.push(line.to_string());
        }
        format!("{header}{}\n{tail}", new_lines.join("\n"))
    })
    .into_owned()
}

fn field_name_from_decl(stripped: &str) -> Option<&str> {
    let no_comment = stripped.split("//").next()?.trim();
    if !no_comment.ends_with(';') {
        return None;
    }
    let no_semi = no_comment.trim_end_matches(';').trim();
    no_semi.split_whitespace().last()
}

/// Injeta `#[dds_typename("module::Struct")]` antes de cada struct listado.
///
/// O derive `DdsTypeDerive` (com suporte a `dds_typename`) emite o typename
/// qualificado usado por C++ (`m_typename`) e Python (`typename=`).
fn qualify_type_names(path: &Path, module: &str, types: &[&str]) {
    if !path.exists() {
        eprintln!("cargo:warning=generated file missing: {}", path.display());
        return;
    }
    let mut src = fs::read_to_string(path).expect("read generated");
    for ty in types {
        // Insert attribute immediately before `pub struct Ty {`
        let needle = format!("pub struct {ty} {{");
        let attr = format!("#[dds_typename(\"{module}::{ty}\")]\npub struct {ty} {{");
        if src.contains(&attr) {
            continue; // already qualified
        }
        if src.contains(&needle) {
            src = src.replacen(&needle, &attr, 1);
        } else {
            eprintln!(
                "cargo:warning=could not inject dds_typename for {module}::{ty} in {}",
                path.display()
            );
        }
    }
    fs::write(path, src).expect("write qualified generated");
}

/// Extrai os blobs `TYPE_INFO_CDR_*`/`TYPE_MAP_CDR_*` do `.c` gerado pelo idlc C
/// e injeta `#[dds_type_metadata(...)]` + constantes `&[u8]` no módulo gerado.
///
/// Com TypeInformation nos endpoints, o SEDP anuncia os TypeIds e peers que
/// exigem validação de tipo (cyclonedds-python, llama-server C++) aceitam o
/// matching (XTypes spec 7.6.3.4 — type consistency enforcement).
fn inject_type_metadata(path: &Path, module: &str, types: &[&str], c_path: &Path) {
    if !path.exists() {
        return;
    }
    let c_src = match fs::read_to_string(c_path) {
        Ok(s) => s,
        Err(e) => {
            eprintln!(
                "cargo:warning=não foi possível ler {}: {e} (type metadata não injetado)",
                c_path.display()
            );
            return;
        }
    };
    let mut src = fs::read_to_string(path).expect("read generated");

    let mut consts = String::new();
    for ty in types {
        let base = to_screaming_snake(ty);
        let (Some(info), Some(map)) = (
            extract_blob(&c_src, module, ty, "TYPE_INFO_CDR"),
            extract_blob(&c_src, module, ty, "TYPE_MAP_CDR"),
        ) else {
            eprintln!("cargo:warning=blobs idlc não encontrados para {module}::{ty}");
            continue;
        };
        consts.push_str(&format!(
            "pub const {base}_TYPE_INFO: &[u8] = &{info:?};\n\
             pub const {base}_TYPE_MAP: &[u8] = &{map:?};\n"
        ));
        let needle = format!("#[dds_typename(\"{module}::{ty}\")]\npub struct {ty} {{");
        let attr = format!(
            "#[dds_typename(\"{module}::{ty}\")]\n\
             #[dds_type_metadata(info = \"{base}_TYPE_INFO\", map = \"{base}_TYPE_MAP\")]\n\
             pub struct {ty} {{"
        );
        if src.contains(&attr) {
            continue;
        }
        if src.contains(&needle) {
            src = src.replacen(&needle, &attr, 1);
        } else {
            eprintln!("cargo:warning=não injetei dds_type_metadata para {module}::{ty}");
        }
    }

    // Insere as constantes no escopo do módulo (logo após `use super::*;`).
    let anchor = "use super::*;";
    if let Some(pos) = src.find(anchor) {
        src.insert_str(pos + anchor.len(), &format!("\n\n{consts}"));
    } else {
        eprintln!(
            "cargo:warning=âncora 'use super::*;' não encontrada em {}",
            path.display()
        );
    }
    fs::write(path, src).expect("write metadata generated");
}

/// Extrai o array de bytes de `#define KIND_<module>_<Type> (const unsigned char []){ ... }`.
fn extract_blob(c_src: &str, module: &str, ty: &str, kind: &str) -> Option<Vec<u8>> {
    let marker = format!("#define {kind}_{module}_{ty} (const unsigned char []){{");
    let start = c_src.find(&marker)? + marker.len();
    let rest = &c_src[start..];
    let end = rest.find("\n}")?;
    let body = &rest[..end];
    let re = Regex::new(r"0x([0-9a-fA-F]{2})").unwrap();
    let bytes: Vec<u8> = re
        .captures_iter(body)
        .map(|c| u8::from_str_radix(&c[1], 16).unwrap())
        .collect();
    if bytes.is_empty() {
        None
    } else {
        Some(bytes)
    }
}

/// `LLMInferenceRequest` → `LLM_INFERENCE_REQUEST`; `TaskOutput` → `TASK_OUTPUT`.
fn to_screaming_snake(name: &str) -> String {
    let chars: Vec<char> = name.chars().collect();
    let mut out = String::new();
    for (i, &ch) in chars.iter().enumerate() {
        if ch.is_uppercase() && i > 0 {
            let prev_lower = chars[i - 1].is_lowercase();
            let prev_upper = chars[i - 1].is_uppercase();
            let next_lower = chars.get(i + 1).is_some_and(|c| c.is_lowercase());
            if prev_lower || (prev_upper && next_lower) {
                out.push('_');
            }
        }
        out.push(ch.to_ascii_uppercase());
    }
    out
}
