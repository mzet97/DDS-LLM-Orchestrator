#!/usr/bin/env python3
"""T-900-M1-02 — gera o módulo mock Rust (build sem `dds`) do WorkflowPlanning.

Lê `WorkflowPlanning.schema.json` (fixture congelada) e emite o corpo de
`dds_llm_workflow_v1` com structs campo-a-campo idênticas ao gerado real:
enums viram i32 (emenda 3), strings viram String, sequences viram Vec<T>.
O gate `mock_workflow_types_match_idl_fixture` (tests/workflow_planning_fixture.rs)
confere a paridade contra a mesma fixture — este script só evita transcrição
manual; a fonte da verdade é a fixture + IDL.

Uso: python3 m1_02_mock_gen.py <schema.json> [saida.rs]
"""
import json
import sys

RUST = {"string": "String", "u16": "u16", "u32": "u32", "u64": "u64",
        "i32": "i32", "f64": "f64", "bool": "bool"}
ENUM_ENUM = {  # nome -> (prefixo doc, comentário curto)
    "IntentCommand": "Workflow.Intent: SUBMIT/CANCEL",
    "QosIntent": "objetivo semântico de QoS (ADR-002)",
    "DecisionStatus": "OK/ERROR",
    "ReasonCode": "causas tipadas (ordem congelada)",
    "NodeKind": "INFERENCE/TOOL",
    "WorkflowStatus": "ciclo de vida do workflow",
    "NodeStatus": "estado por nó",
}


def rust_type(f: dict) -> str:
    t = f["type"]
    if t in RUST:
        return RUST[t]
    if t == "seq":
        return f"Vec<{rust_type(f['elem'])}>"
    if t == "enum":
        return "i32"  # emenda 3: campo carrega o ordinal
    if t == "struct":
        return f["struct"]
    raise SystemExit(f"tipo desconhecido: {t}")


def main() -> None:
    schema = json.load(open(sys.argv[1]))
    out = [f"// GERADO por m1_02_mock_gen.py a partir de {sys.argv[1]} — não editar à mão.",
           "// Mock do build sem `dds`; a paridade com o IDL é travada por teste.",
           ""]
    for name, spec in schema["enums"].items():
        out.append(f"/// {ENUM_ENUM.get(name, name)} — ordinais canônicos (emenda 3: campos usam i32).")
        out.append("#[derive(Debug, Clone, Copy, PartialEq, Eq)]")
        out.append("#[repr(i32)]")
        out.append(f"pub enum {name} {{")
        for i, lit in enumerate(spec["literals"]):
            sep = "," if i < len(spec["literals"]) - 1 else ""
            out.append(f"    {lit} = {i}{sep}")
        out.append("}")
        out.append("")
    for name, spec in schema["structs"].items():
        keys = {f["name"] for f in spec["fields"] if f.get("key")}
        if keys:
            out.append(f"/// Chaves lógicas (IDL @key): {', '.join(sorted(keys))}.")
        out.append("#[derive(Debug, Clone, Default, PartialEq, serde::Serialize, serde::Deserialize)]")
        out.append(f"pub struct {name} {{")
        for f in spec["fields"]:
            doc = []
            if "bound" in f and f["type"] == "string":
                doc.append(f"bound {f['bound']}")
            if f["type"] == "seq" and "bound" in f:
                doc.append(f"bound seq {f['bound']}")
            comment = f"  // {', '.join(doc)}" if doc else ""
            out.append(f"    pub {f['name']}: {rust_type(f)},{comment}")
        out.append("}")
        out.append("")
    text = "\n".join(out)
    if len(sys.argv) > 2:
        open(sys.argv[2], "w").write(text)
        print(f"WROTE {sys.argv[2]} ({len(text)} bytes)")
    else:
        print(text)


if __name__ == "__main__":
    main()
