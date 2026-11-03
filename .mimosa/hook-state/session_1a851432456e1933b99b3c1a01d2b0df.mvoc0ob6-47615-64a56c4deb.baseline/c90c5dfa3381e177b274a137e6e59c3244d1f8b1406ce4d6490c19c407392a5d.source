"""Encoding de digest do contrato (contracts.md §Compatibilidade).

Preimage documental (NÃO é CDR): campos na ordem declarada excluindo o
próprio campo hash; string=u32 BE len + UTF-8; inteiros na largura IDL BE;
enum=ordinal u32 (ordem declarada); bool=u8; double=IEEE-754 BE com -0
normalizado e NaN/inf rejeitados; seq=u32 count + elementos; struct recursiva.

Validação cruzada Rust==Python: os 16 vetores golden em
`crates/dds-contract/idl/WorkflowPlanning.digest_vectors.json` (teste
`digest_vectors_reproduce_in_rust` no dds-contract; teste
`test_digest.py` aqui reproduz os mesmos hashes).
"""

from __future__ import annotations

import hashlib
import math
import struct
from typing import Any

# Ordinais canônicos (emenda 3: campos carregam i32 com estes ordinais).
ENUM_LITERALS: dict[str, list[str]] = {
    "IntentCommand": ["IC_SUBMIT", "IC_CANCEL"],
    "QosIntent": ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"],
    "DecisionStatus": ["DS_OK", "DS_ERROR"],
    "ReasonCode": [
        "RC_NONE", "RC_INVALID_INPUT", "RC_MODEL_UNAVAILABLE", "RC_TIMEOUT",
        "RC_SCHEMA_MISMATCH", "RC_POLICY_DENIED", "RC_LOW_CONFIDENCE", "RC_EXPIRED",
        "RC_CAPABILITY_UNAVAILABLE", "RC_OVERLOADED", "RC_CONFLICT", "RC_INVALID_PLAN",
        "RC_OUTPUT_INVALID", "RC_IN_DOUBT", "RC_CANCELLED",
    ],
    "NodeKind": ["NK_INFERENCE", "NK_TOOL"],
    "WorkflowStatus": [
        "WS_ACCEPTED", "WS_WAITING_DECISION", "WS_WAITING_PLAN", "WS_RUNNING",
        "WS_DONE", "WS_FAILED", "WS_CANCELLED", "WS_IN_DOUBT",
    ],
    "NodeStatus": [
        "NS_BLOCKED", "NS_READY", "NS_PUBLISHED", "NS_ASSIGNED", "NS_RUNNING",
        "NS_DONE", "NS_FAILED", "NS_CANCELLED", "NS_IN_DOUBT",
    ],
}


def _pack(out: bytearray, fmt: str, value: Any) -> None:
    out += struct.pack(fmt, value)


def _f64(out: bytearray, value: float) -> None:
    if math.isnan(value) or math.isinf(value):
        raise ValueError("NaN/infinito rejeitado no digest")
    if value == 0.0:
        value = 0.0  # normaliza -0.0 -> +0.0
    _pack(out, ">d", value)


def encode_typed(tv: dict[str, Any], out: bytearray) -> None:
    t, v = tv["t"], tv["v"]
    if t == "string":
        b = v.encode("utf-8")
        _pack(out, ">I", len(b))
        out += b
    elif t == "u16":
        _pack(out, ">H", v)
    elif t == "u32":
        _pack(out, ">I", v)
    elif t == "u64":
        _pack(out, ">Q", v)
    elif t == "i32":
        _pack(out, ">i", v)
    elif t == "bool":
        _pack(out, ">B", 1 if v else 0)
    elif t == "f64":
        _f64(out, float(v))
    elif t == "enum":
        _pack(out, ">I", ENUM_LITERALS[tv["enum"]].index(v))
    elif t == "seq":
        _pack(out, ">I", len(v))
        for item in v:
            encode_typed(item, out)
    elif t == "struct":
        from . import contract_schema

        for fld in contract_schema.STRUCT_FIELDS[tv["struct"]]:
            encode_typed(v[fld], out)
    else:
        raise ValueError(f"tipo desconhecido: {t}")


def digest_typed(tv: dict[str, Any], skip_field: str | None = None) -> str:
    """SHA-256 hex da preimage (exclui `skip_field` — o próprio campo hash)."""
    out = bytearray()
    if tv["t"] == "struct":
        from . import contract_schema

        for fld in contract_schema.STRUCT_FIELDS[tv["struct"]]:
            if fld == skip_field:
                continue
            encode_typed(tv["v"][fld], out)
    else:
        encode_typed(tv, out)
    return hashlib.sha256(bytes(out)).hexdigest()
