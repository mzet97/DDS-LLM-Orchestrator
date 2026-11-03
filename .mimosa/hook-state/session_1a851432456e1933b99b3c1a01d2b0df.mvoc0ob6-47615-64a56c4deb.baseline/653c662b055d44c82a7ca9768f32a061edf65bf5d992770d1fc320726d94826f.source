#!/usr/bin/env python3
"""Valida um chunk gerado: formato SDK (state/questions/expected/tags-lista),
idioma, dedup intra-arquivo. Exit != 0 se inválido."""
import json, re, sys
from pathlib import Path

CAPS = ["rust_refactor","python_refactor","unit_testing","security_audit",
        "code_review","research","summarization","translation"]

def norm(t): return re.sub(r"\s+", " ", t.lower()).strip()[:120]

def main():
    path = Path(sys.argv[1])
    rows = [json.loads(l) for l in path.read_text().splitlines() if l.strip()]
    seen, bad = set(), []
    for i, r in enumerate(rows):
        if set(r) < {"state","questions","expected"}: bad.append((i,"keys")); continue
        if not isinstance(r["tags"], list): bad.append((i,"tags")); continue
        if r["state"].get("text","").strip()=="" : bad.append((i,"vazio")); continue
        k = norm(r["state"]["text"])
        if k in seen: bad.append((i,"dup")); continue
        seen.add(k)
        exp = r["expected"]
        if exp.get("qos_profile","") not in ("QI_BALANCED","QI_CRITICAL","QI_STREAM_LIKE","QI_LOW_COST","QI_FAILOVER"):
            bad.append((i,"qos"))
    if bad:
        print(f"INVÁLIDO: {len(bad)} problemas em {path}: {bad[:5]}"); return 1
    print(f"OK: {len(rows)} exemplos únicos e bem-formados em {path}")
    return 0

if __name__ == "__main__":
    sys.exit(main())
