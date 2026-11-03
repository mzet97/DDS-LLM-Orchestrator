#!/usr/bin/env python3
"""T-900-M1-01 — ferramenta de fixtures do contrato WorkflowPlanning.idl.

Funções (mutuamente exclusivas, via argv[1]):

  schema   Parseia o IDL canônico e emite/verifica WorkflowPlanning.schema.json.
  digest   Recalcula os vetores golden do encoding de digest (payload_hash/
           plan_hash) conforme contracts.md §Compatibilidade (M0-1).
  check-py Confere código Python gerado (idlpy) contra o schema fixture.
  check-rs Confere código Rust gerado (cyclonedds-build) contra o schema fixture.

O encoding de digest NÃO é CDR: é a preimage documental (u32 BE, big-endian,
UTF-8, -0 normalizado, NaN/inf rejeitados) usada apenas para hashes de
integridade de contrato. Wire contínuo usando XCDR1 é validado em M1-04/V02.

Uso:
  python3 m1_01_fixture_tool.py schema <idl> [--write <out.json>] [--check <out.json>]
  python3 m1_01_fixture_tool.py digest <vectors.json> [--write] [--check]
  python3 m1_01_fixture_tool.py check-py <schema.json> <dir_gerado>
  python3 m1_01_fixture_tool.py check-rs <schema.json> <arquivo.rs>
"""
from __future__ import annotations

import hashlib
import json
import re
import struct
import sys
from pathlib import Path

PRIM = {
    "unsigned short": "u16",
    "unsigned long": "u32",
    "unsigned long long": "u64",
    "long": "i32",
    "double": "f64",
    "boolean": "bool",
}

# ---------------------------------------------------------------- IDL parse

def strip_comments(text: str) -> str:
    text = re.sub(r"/\*.*?\*/", "", text, flags=re.S)
    return re.sub(r"//[^\n]*", "", text)


ENUM_NAMES: set = set()
STRUCT_NAMES: set = set()


def parse_idl(path: Path) -> dict:
    global ENUM_NAMES, STRUCT_NAMES
    src = strip_comments(path.read_text(encoding="utf-8"))
    mmod = re.search(r"module\s+(\w+)\s*\{", src)
    if not mmod:
        raise SystemExit("module não encontrado")
    module = mmod.group(1)
    body = src[mmod.end():]

    enums: dict[str, list[str]] = {}
    for m in re.finditer(r"enum\s+(\w+)\s*\{([^}]*)\}", body):
        literals = [l.strip() for l in m.group(2).split(",") if l.strip()]
        enums[m.group(1)] = literals
    ENUM_NAMES = set(enums)
    # pré-scan dos nomes de struct: parse_type resolve referências aninhadas
    STRUCT_NAMES = set(re.findall(r"\bstruct\s+(\w+)\s*\{", body))

    structs: dict[str, dict] = {}
    for m in re.finditer(r"(?:@final\s*)?struct\s+(\w+)\s*\{(.*?)\n\s*\};", body, re.S):
        name, sbody = m.group(1), m.group(2)
        # @key pode vir na própria linha do campo ("@key string<64> request_id;")
        # ou em linha anterior; suportamos ambas.
        fields = []
        key_pending = False
        for raw in sbody.split("\n"):
            line = raw.strip()
            if not line:
                continue
            if line.startswith("@key"):
                line = line[len("@key"):].strip()
                key_pending = True
                if not line:
                    continue
            if line.startswith("@"):
                continue
            fm = re.match(r"(.+?)\s+(\w+);$", line)
            if not fm:
                raise SystemExit(f"campo não parseado em {name}: {line!r}")
            tdecl, fname = fm.group(1).strip(), fm.group(2)
            fields.append({"name": fname, **parse_type(tdecl), **({"key": True} if key_pending else {})})
            key_pending = False
        structs[name] = {
            "final": bool(re.search(r"@final\s+struct\s+" + name + r"\b", body)),
            "fields": fields,
        }
    return {"module": module, "enums": enums, "structs": structs}


def parse_type(decl: str) -> dict:
    decl = decl.strip()
    if decl in PRIM:
        return {"type": PRIM[decl]}
    m = re.match(r"string(?:<(\d+)>)?$", decl)
    if m:
        t = {"type": "string"}
        if m.group(1):
            t["bound"] = int(m.group(1))
        return t
    m = re.match(r"sequence<\s*(.+?)\s*(?:,\s*(\d+)\s*)?>$", decl)
    if m:
        t = {"type": "seq", "elem": parse_type(m.group(1))}
        if m.group(2):
            t["bound"] = int(m.group(2))
        return t
    if decl in ENUM_NAMES:
        return {"type": "enum", "enum": decl}
    if decl in STRUCT_NAMES:
        return {"type": "struct", "struct": decl}
    raise SystemExit(f"tipo não reconhecido: {decl!r}")


# ---------------------------------------------------------------- schema fixture

TOPICS = [
    ("Workflow.Intent", "WorkflowIntent", ["request_id"]),
    ("Routing.Decision", "RoutingDecision", ["request_id", "decision_id"]),
    ("Agent.Capabilities", "AgentCapabilities", ["agent_id"]),
    ("Workflow.Plan", "WorkflowPlan", ["workflow_id", "plan_revision"]),
    ("Workflow.State", "WorkflowState", ["workflow_id", "plan_revision"]),
]

NOTES = [
    "EMENDA M1-01 (2026-10-16): literais de enum prefixados (IC_/QI_/DS_/RC_/NK_/WS_/NS_) "
    "por regra de escopo IDL — idlc C 11.0.1 rejeita literais duplicados no módulo "
    "(ex.: CANCELLED em ReasonCode/WorkflowStatus/NodeStatus). Ordinais/semântica idênticos ao contracts.md.",
    "schema_version: valor OBRIGATÓRIO 1 em V1 garantido por validação (IDL 4.2 não tem default de membro; "
    "ambos geradores rejeitam `= 1`).",
    "Divergência conhecida: cyclonedds-build 3.0.1 gera DdsString sem bound; bounds de sequence preservados. "
    "idlc C e idlpy preservam bounds de string.",
]


def build_schema(idl: dict) -> dict:
    return {
        "fixture_version": 1,
        "task": "T-900-M1-01",
        "module": idl["module"],
        "topics": [
            {"topic": t, "typename": f'{idl["module"]}::{s}', "struct": s, "keys": k}
            for t, s, k in TOPICS
        ],
        "enums": {name: {"literals": lits} for name, lits in idl["enums"].items()},
        "structs": idl["structs"],
        "notes": NOTES,
    }


def verify_keys_order(schema: dict) -> None:
    for entry in schema["topics"]:
        st = schema["structs"][entry["struct"]]
        key_fields = [f["name"] for f in st["fields"] if f.get("key")]
        assert key_fields == entry["keys"], (entry["struct"], key_fields, entry["keys"])


# ---------------------------------------------------------------- digest encoding

def enc_typed(tv: dict, schema: dict, out: bytearray) -> None:
    """Codifica um VALOR TIPADO ({"t":..., "v":...}) na preimage documental.

    A ordem dos campos de struct vem do schema fixture; ordinais de enum são
    resolvidos pela posição do literal no schema (ordem declarada no IDL).
    """
    t, val = tv["t"], tv["v"]
    if t == "string":
        b = val.encode("utf-8")
        out += struct.pack(">I", len(b)) + b
    elif t == "u16":
        out += struct.pack(">H", val)
    elif t == "u32":
        out += struct.pack(">I", val)
    elif t == "u64":
        out += struct.pack(">Q", val)
    elif t == "i32":
        out += struct.pack(">i", val)
    elif t == "bool":
        out += struct.pack(">B", 1 if val else 0)
    elif t == "f64":
        f = float(val)
        if f != f or f in (float("inf"), float("-inf")):
            raise ValueError("NaN/infinito rejeitado no digest")
        if f == 0.0:  # normaliza -0.0 -> +0.0
            f = 0.0
        out += struct.pack(">d", f)
    elif t == "enum":
        literals = schema["enums"][tv["enum"]]["literals"]
        out += struct.pack(">I", literals.index(val))
    elif t == "seq":
        out += struct.pack(">I", len(val))
        for item in val:
            enc_typed(item, schema, out)
    elif t == "struct":
        st = schema["structs"][tv["struct"]]
        for fld in st["fields"]:
            enc_typed(val[fld["name"]], schema, out)
    else:
        raise ValueError(f"tipo desconhecido: {t}")


def digest_vector(tv: dict, schema: dict, skip_field: str | None) -> tuple[str, str]:
    out = bytearray()
    if tv["t"] == "struct":
        st = schema["structs"][tv["struct"]]
        for fld in st["fields"]:
            if fld["name"] == skip_field:
                continue
            enc_typed(tv["v"][fld["name"]], schema, out)
    else:
        enc_typed(tv, schema, out)
    pre = bytes(out)
    return pre.hex(), hashlib.sha256(pre).hexdigest()


def V(t: str, v, enum: str | None = None, elem: dict | None = None, struct: str | None = None):
    d = {"t": t, "v": v}
    if enum:
        d["enum"] = enum
    if elem:
        d["elem"] = elem
    if struct:
        d["struct"] = struct
    return d


def build_vectors(schema: dict) -> dict:
    S = lambda s: V("string", s)
    I = lambda t, n: V(t, n)
    E = lambda e, l: V("enum", l, enum=e)
    # SEQ recebe items já tipados
    def SEQ(elem_tv, items):
        return V("seq", items, elem=elem_tv)

    hex64 = "ab" * 32  # 64 chars: digest-like
    intent_val = {
        "request_id": S("req-0001"),
        "workflow_id": S("wf-0001"),
        "client_id": S("cli-01"),
        "schema_version": I("u16", 1),
        "intent_revision": I("u32", 1),
        "command": E("IntentCommand", "IC_SUBMIT"),
        "expected_plan_revision": I("u32", 0),
        "prompt": S("Audit this Rust service and create tests"),
        "created_at_ns": I("u64", 1760000000_000000001),
        "deadline_ns": I("u64", 1760000600_000000001),
        "security_context_ref": S("ctx://tenant-a/policy-3"),
        "policy_revision": I("u32", 7),
        "questions_revision": I("u32", 2),
        "capability_catalog_revision": I("u32", 3),
        "checkpoint_revision": S(hex64),
        "max_tokens": I("u32", 8192),
        "max_cost_microunits": I("u64", 1_500_000),
        "budget_unit": S("MICRO_USD"),
        "fallback_template_id": S(""),
        "payload_hash": S(hex64),
    }
    plan_node_val = lambda n, cap: {
        "node_id": S(n),
        "kind": E("NodeKind", "NK_INFERENCE"),
        "required_capabilities": SEQ(V("string", cap), [V("string", cap)]),
        "model_id": V("string", "gemma-4-12b-it"),
        "target_agent_id": S(""),
        "prompt_template": S("Summarize the audit report"),
        "dependencies": SEQ(V("string", "node-a"), [V("string", "node-a")]),
        "inputs": SEQ(
            V("struct", {
                "producer_node_id": V("string", "node-a"),
                "output_name": V("string", "report"),
            }, struct="InputRef"),
            [V("struct", {
                "producer_node_id": V("string", "node-a"),
                "output_name": V("string", "report"),
            }, struct="InputRef")],
        ),
        "output_names": SEQ(V("string", "summary"), [V("string", "summary")]),
        "max_attempts": I("u16", 2),
        "deadline_ns": I("u64", 1760000500_000000001),
        "max_tokens": I("u32", 1024),
        "max_output_bytes": I("u32", 65536),
        "security_context_ref": S("ctx://tenant-a/policy-3"),
        "priority": I("i32", 5),
        "side_effect_key": S(""),
    }
    plan_val = {
        "workflow_id": S("wf-0001"),
        "plan_revision": I("u32", 1),
        "schema_version": I("u16", 1),
        "request_id": S("req-0001"),
        "intent_revision": I("u32", 1),
        "effective_decision_id": S("dec-0001"),
        "policy_revision": I("u32", 7),
        "capability_catalog_revision": I("u32", 3),
        "planner_id": S("static-template"),
        "planner_revision": S(hex64),
        "created_at_ns": I("u64", 1760000010_000000001),
        "deadline_ns": I("u64", 1760000600_000000001),
        "qos_profile": E("QosIntent", "QI_CRITICAL"),
        "nodes": SEQ(V("struct", plan_node_val("node-b", "rust_refactor"), struct="PlanNode"),
                     [V("struct", plan_node_val("node-b", "rust_refactor"), struct="PlanNode")]),
        "plan_hash": S(hex64),
    }
    hex64b = "cd" * 32
    decision_val = {
        "request_id": S("req-0001"),
        "decision_id": S("dec-0001"),
        "workflow_id": S("wf-0001"),
        "schema_version": I("u16", 1),
        "intent_revision": I("u32", 1),
        "producer_id": S("laya-worker-1"),
        "model_id": S("convaiinnovations/laya"),
        "checkpoint_revision": S(hex64),
        "questions_revision": I("u32", 2),
        "policy_revision": I("u32", 7),
        "capability_catalog_revision": I("u32", 3),
        "qos_profile": E("QosIntent", "QI_CRITICAL"),
        "qos_probabilities": SEQ(
            V("struct", {"profile": E("QosIntent", "QI_BALANCED"), "probability": V("f64", 0.12)}, struct="ProfileProbability"),
            [V("struct", {"profile": E("QosIntent", "QI_BALANCED"), "probability": V("f64", 0.12)}, struct="ProfileProbability"),
             V("struct", {"profile": E("QosIntent", "QI_CRITICAL"), "probability": V("f64", 0.73)}, struct="ProfileProbability"),
             V("struct", {"profile": E("QosIntent", "QI_STREAM_LIKE"), "probability": V("f64", 0.05)}, struct="ProfileProbability"),
             V("struct", {"profile": E("QosIntent", "QI_LOW_COST"), "probability": V("f64", 0.04)}, struct="ProfileProbability"),
             V("struct", {"profile": E("QosIntent", "QI_FAILOVER"), "probability": V("f64", 0.06)}, struct="ProfileProbability")],
        ),
        "capability_probabilities": SEQ(
            V("struct", {"capability_id": V("string", "rust_refactor"), "probability": V("f64", 0.91),
                         "answer_confidence": V("f64", 0.77)}, struct="CapabilityProbability"),
            [V("struct", {"capability_id": V("string", "rust_refactor"), "probability": V("f64", 0.91),
                          "answer_confidence": V("f64", 0.77)}, struct="CapabilityProbability"),
             V("struct", {"capability_id": V("string", "unit_testing"), "probability": V("f64", 0.64),
                          "answer_confidence": V("f64", 0.58)}, struct="CapabilityProbability")],
        ),
        "requires_planning_probability": V("f64", 0.31),
        "priority_score": V("f64", 1.28),
        "priority_probabilities": SEQ(V("f64", 0.0), [V("f64", 0.18), V("f64", 0.70), V("f64", 0.12)]),
        "confidence": V("f64", 0.71),
        "answer_confidence": V("f64", 0.66),
        "calibration_revision": S(""),
        "created_at_ns": I("u64", 1760000005_000000001),
        "valid_until_ns": I("u64", 1760000300_000000001),
        "decision_status": E("DecisionStatus", "DS_OK"),
        "reason_code": E("ReasonCode", "RC_NONE"),
        "evidence_digest": S(hex64b),
        "payload_hash": S(hex64b),
    }
    vectors = [
        ("string-basic", "string UTF-8 com comprimento u32 BE", V("string", "laya"), S("laya")),
        ("string-empty", "string vazia -> apenas comprimento 0", V("string", ""), S("")),
        ("u16", "unsigned short (2 bytes BE)", I("u16", 1), I("u16", 1)),
        ("u32", "unsigned long (4 bytes BE)", I("u32", 8192), I("u32", 8192)),
        ("u64", "unsigned long long (8 bytes BE)", I("u64", 1760000000_000000001), I("u64", 1760000000_000000001)),
        ("i32", "long signed (4 bytes BE, complemento-2)", I("i32", -10), I("i32", -10)),
        ("bool", "boolean u8 0/1", V("bool", True), V("bool", True)),
        ("f64", "double IEEE-754 BE", V("f64", 0.25), V("f64", 0.25)),
        ("f64-negzero", "-0.0 normalizado para +0.0", V("f64", -0.0), V("f64", -0.0)),
        ("enum-ordinal", "enum -> ordinal u32 BE na ordem declarada", E("QosIntent", "QI_STREAM_LIKE"), E("QosIntent", "QI_STREAM_LIKE")),
        ("seq-empty", "sequência vazia -> comprimento 0", SEQ(V("string", "x"), []), SEQ(V("string", "x"), [])),
        ("seq-doubles", "sequence<double,3> -> u32 count + doubles BE", SEQ(V("f64", 0.0), [V("f64", 1.5), V("f64", 2.5)]), SEQ(V("f64", 0.0), [V("f64", 1.5), V("f64", 2.5)])),
        ("struct-nested", "struct composta recursiva (ProfileProbability)", V("struct", {
            "profile": E("QosIntent", "QI_CRITICAL"),
            "probability": V("f64", 0.42),
        }, struct="ProfileProbability"), V("struct", {
            "profile": E("QosIntent", "QI_CRITICAL"),
            "probability": V("f64", 0.42),
        }, struct="ProfileProbability")),
        ("intent-full", "WorkflowIntent completa (payload_hash EXCLUÍDO da preimage)",
         V("struct", intent_val, struct="WorkflowIntent"), None),
        ("plan-full", "WorkflowPlan completa (plan_hash EXCLUÍDO da preimage)",
         V("struct", plan_val, struct="WorkflowPlan"), None),
        ("decision-full", "RoutingDecision completa (payload_hash EXCLUÍDO da preimage)",
         V("struct", decision_val, struct="RoutingDecision"), None),
    ]
    out = []
    for vid, desc, tv, _dup in vectors:
        skip = {"intent-full": "payload_hash", "plan-full": "plan_hash", "decision-full": "payload_hash"}.get(vid)
        pre, sha = digest_vector(tv, schema, skip)
        out.append({"id": vid, "desc": desc, "value": tv, "skip_field": skip,
                    "preimage_hex": pre, "sha256": sha})
    return {
        "fixture_version": 1,
        "task": "T-900-M1-01",
        "encoding": "contracts.md M0-1 §Compatibilidade: campos na ordem declarada excluindo o próprio campo hash; "
                    "string=u32 BE len + UTF-8; inteiros na largura IDL BE; enum=ordinal u32; bool=u8; "
                    "double=IEEE-754 BE com -0 normalizado e NaN/inf rejeitados; seq=u32 count + elementos; "
                    "struct=campos recursivos. Preimage documental, NÃO é CDR do wire.",
        "vectors": out,
    }


# ---------------------------------------------------------------- generated checks

def check_py(schema: dict, gen_dir: Path) -> list[str]:
    issues = []
    text = (gen_dir / "_WorkflowPlanning.py").read_text(encoding="utf-8")
    for ename, espec in schema["enums"].items():
        block = re.search(
            rf"class {ename}\(idl\.IdlEnum[^\n]*\):\n((?:[ \t]+\w+ = auto\(\)\n?)*)", text)
        if not block:
            issues.append(f"enum {ename} ausente no Python gerado")
            continue
        got = re.findall(r"^[ \t]+(\w+) = auto\(\)", block.group(1), re.M)
        if got != espec["literals"]:
            issues.append(f"enum {ename}: {got} != {espec['literals']}")
    for sname, sspec in schema["structs"].items():
        block = re.search(rf"class {sname}\(idl\.IdlStruct.*?\):(.*?)(?=\n\n|\Z)", text, re.S)
        if not block:
            issues.append(f"struct {sname} ausente no Python gerado")
            continue
        body = block.group(1)
        got_fields = re.findall(r"^\s{4}(\w+): (.+)$", body, re.M)
        want = [f["name"] for f in sspec["fields"]]
        got = [g[0] for g in got_fields]
        if got != want:
            issues.append(f"struct {sname} campos: {got} != {want}")
        # keys
        keys_got = re.findall(r'annotate\.key\("(\w+)"\)', body)
        keys_want = [f["name"] for f in sspec["fields"] if f.get("key")]
        if keys_got != keys_want:
            issues.append(f"struct {sname} keys: {keys_got} != {keys_want}")
        # bounds de string e sequence
        for fld in sspec["fields"]:
            line = next((g[1] for g in got_fields if g[0] == fld["name"]), "")
            if fld["type"] == "string" and "bound" in fld:
                if line != f"types.bounded_str[{fld['bound']}]":
                    issues.append(f"{sname}.{fld['name']}: esperado bounded_str[{fld['bound']}], got {line}")
            if fld["type"] == "seq" and "bound" in fld:
                b = fld["bound"]
                if f", {b}]" not in line:
                    issues.append(f"{sname}.{fld['name']}: bound seq {b} ausente em {line}")
    return issues


def check_rs(schema: dict, gen_rs: Path) -> list[str]:
    issues = []
    text = gen_rs.read_text(encoding="utf-8")
    for ename, espec in schema["enums"].items():
        block = re.search(rf"pub enum {ename} \{{(.*?)\}}", text, re.S)
        if not block:
            issues.append(f"enum {ename} ausente no Rust gerado")
            continue
        got = re.findall(r"^\s{8}(\w+)(?: = \d+)?,", block.group(1), re.M)
        if got != espec["literals"]:
            issues.append(f"enum {ename}: {got} != {espec['literals']}")
    for sname, sspec in schema["structs"].items():
        block = re.search(rf"pub struct {sname} \{{(.*?)\}}", text, re.S)
        if not block:
            issues.append(f"struct {sname} ausente no Rust gerado")
            continue
        body = block.group(1)
        got = re.findall(r"pub (\w+):", body)
        want = [f["name"] for f in sspec["fields"]]
        if got != want:
            issues.append(f"struct {sname} campos: {got} != {want}")
        keys_got = []
        lines = body.split("\n")
        for i, ln in enumerate(lines):
            km = re.match(r"\s*#\[key\]", ln)
            if km and i + 1 < len(lines):
                fm = re.search(r"pub (\w+):", lines[i + 1])
                if fm:
                    keys_got.append(fm.group(1))
        keys_want = [f["name"] for f in sspec["fields"] if f.get("key")]
        if keys_got != keys_want:
            issues.append(f"struct {sname} keys: {keys_got} != {keys_want}")
        for fld in sspec["fields"]:
            if fld["type"] == "seq" and "bound" in fld:
                pat = rf"pub {fld['name']}: DdsBoundedSequence<.+?, {fld['bound']}>"
                if not re.search(pat, text):
                    issues.append(f"{sname}.{fld['name']}: DdsBoundedSequence bound {fld['bound']} ausente")
    return issues


# ---------------------------------------------------------------- main

def main() -> int:
    args = sys.argv[1:]
    cmd = args[0]
    if cmd == "schema":
        idl = parse_idl(Path(args[1]))
        ENUM_NAMES = set(idl["enums"])  # noqa: F841 (parse_type usa via closure global)
        schema = build_schema(idl)
        verify_keys_order(schema)
        if "--write" in args:
            Path(args[args.index("--write") + 1]).write_text(
                json.dumps(schema, indent=1, sort_keys=False) + "\n", encoding="utf-8")
            print("WROTE schema fixture")
        if "--check" in args:
            frozen = json.loads(Path(args[args.index("--check") + 1]).read_text())
            if frozen != schema:
                print("SCHEMA MISMATCH: IDL divergiu da fixture congelada")
                for k in ("topics", "enums", "structs", "module"):
                    if frozen.get(k) != schema.get(k):
                        print(f"  divergência em: {k}")
                return 4
            print("SCHEMA CHECK OK (IDL == fixture)")
    elif cmd == "digest":
        schema = json.loads(Path(args[1]).read_text())
        vectors = build_vectors(schema)
        if "--write" in args:
            Path(args[args.index("--write") + 1]).write_text(
                json.dumps(vectors, indent=1) + "\n", encoding="utf-8")
            print("WROTE digest vectors")
        if "--check" in args:
            frozen = json.loads(Path(args[args.index("--check") + 1]).read_text())
            for f, c in zip(frozen["vectors"], vectors["vectors"]):
                if f["sha256"] != c["sha256"] or f["preimage_hex"] != c["preimage_hex"]:
                    print(f"DIGEST MISMATCH {f['id']}")
                    return 5
            print(f"DIGEST CHECK OK ({len(vectors['vectors'])} vetores)")
    elif cmd == "check-py":
        schema = json.loads(Path(args[1]).read_text())
        issues = check_py(schema, Path(args[2]))
        print("PY CHECK:", "OK" if not issues else "FAIL")
        for i in issues:
            print("  -", i)
        return 1 if issues else 0
    elif cmd == "check-rs":
        schema = json.loads(Path(args[1]).read_text())
        issues = check_rs(schema, Path(args[2]))
        print("RS CHECK:", "OK" if not issues else "FAIL")
        for i in issues:
            print("  -", i)
        return 1 if issues else 0
    else:
        print(__doc__)
        return 2
    return 0


if __name__ == "__main__":
    sys.exit(main())
