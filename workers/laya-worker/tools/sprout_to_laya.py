#!/usr/bin/env python3
"""PoC: SPROUT (CARROT-LLM-Routing) → formato de calibração do Laya.

Label POR MEDIÇÃO (não por construção): cada prompt real foi avaliado por
13 modelos com score de juiz LLM; derivamos o rótulo de QoS do tradeoff
custo×qualidade realmente observado (Pareto), e a complexidade da
dificuldade medida. Gates: Dataset.from_jsonl + numeric_targets.
"""
import json
import sys
import urllib.request
from pathlib import Path

sys.path.insert(0, "/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/tools")
sys.path.insert(0, "/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/src")
from gen_dataset import questions_for  # noqa: E402  (schema idêntico ao calibrado)
from targets import numeric_targets  # noqa: E402

API = "https://datasets-server.huggingface.co/rows"
DS = "CARROT-LLM-Routing%2FSPROUT"

FRONTIER = ["openai-gpt-4o", "aws-claude-3-5-sonnet-v1",
            "wxai-llama-3-405b-instruct", "wxai-llama-3-3-70b-instruct"]
MID = ["wxai-llama-3-1-70b-instruct"]
CHEAP = ["openai-gpt-4o-mini", "wxai-llama-3-2-1b-instruct",
         "wxai-llama-3-2-3b-instruct", "wxai-llama-3-1-8b-instruct",
         "wxai-granite-3-2b-instruct-8k-max-tokens",
         "wxai-granite-3-8b-instruct-8k-max-tokens",
         "wxai-mixtral-8x7b-instruct-v01", "aws-titan-text-premier-v1"]


def fetch(n_rows: int) -> list[dict]:
    rows, off = [], 0
    while len(rows) < n_rows:
        url = f"{API}?dataset={DS}&config=default&split=train&offset={off}&length=100"
        with urllib.request.urlopen(url, timeout=60) as r:
            batch = json.load(r).get("rows", [])
        if not batch:
            break
        rows.extend(b["row"] for b in batch)
        off += len(batch)
    return rows[:n_rows]


def scores(row: dict, models: list[str]) -> list[float]:
    out = []
    for m in models:
        v = row.get(m) or {}
        s = v.get("score")
        if isinstance(s, (int, float)):
            out.append(float(s))
    return out


def label_qos(row: dict) -> str:
    cheap_max = max(scores(row, CHEAP) or [0.0])
    mid_max = max(scores(row, MID) or [0.0])
    front_max = max(scores(row, FRONTIER) or [0.0])
    if cheap_max >= 0.95:
        return "QI_LOW_COST"          # barato já resolve com qualidade máxima medida
    if front_max >= 0.90 and front_max - cheap_max >= 0.25:
        return "QI_CRITICAL"          # gap grande: só rota premium alcança 0.9+
    if mid_max >= 0.90:
        return "QI_BALANCED"          # meio resolve, barato não
    return "QI_BALANCED"              # neutro honesto (nenhum tier claro)


def label_complexity(row: dict) -> int:
    allsc = scores(row, FRONTIER + MID + CHEAP) or [0.0]
    mean, mx = sum(allsc) / len(allsc), max(allsc)
    if mx < 0.5:
        return 3                       # nem frontier resolve → very_complex
    if mean >= 0.85:
        return 0
    if mean >= 0.70:
        return 1
    return 2


def convert(row: dict) -> dict:
    src = row.get("dataset", "")
    planning = src.startswith("lighteval/MATH")
    caps = {f"cap__{c}": False for c in
            ["rust_refactor", "python_refactor", "unit_testing", "security_audit",
             "code_review", "summarization", "translation"]}
    caps["cap__research"] = "ragbench" in src  # QA sobre corpus ≈ levantamento
    qos = label_qos(row)
    expected = {
        "qos_profile": qos,
        "needs_planning": planning,
        "complexity": label_complexity(row),
        "urgency": 1,  # sem sinal real no SPROUT — prior neutra (documentado)
        **caps,
    }
    return {
        "state": {"text": row["prompt"]},
        "questions": questions_for(),
        "expected": expected,
        "language": "en",
        "tags": ["lang:en", f"qos:{qos}", f"src:{src}",
                 f"planning:{int(planning)}", "provenance:sprout-pareto-measured"],
    }


def main() -> int:
    n = int(sys.argv[1]) if len(sys.argv) > 1 else 100
    rows = fetch(n)
    print(f"buscadas {len(rows)} linhas SPROUT", flush=True)
    examples = [convert(r) for r in rows]

    out = Path(f"/var/tmp/sprout_poc/sprout_laya_{n}.jsonl")
    out.parent.mkdir(parents=True, exist_ok=True)
    out.write_text("\n".join(json.dumps(e, ensure_ascii=False) for e in examples) + "\n")

    # distribuição dos rótulos derivados
    from collections import Counter
    print("qos:", dict(Counter(e["expected"]["qos_profile"] for e in examples)))
    print("complexity:", dict(Counter(e["expected"]["complexity"] for e in examples)))
    print("planning:", dict(Counter(e["expected"]["needs_planning"] for e in examples)))
    print("research:", dict(Counter(e["expected"]["cap__research"] for e in examples)))

    # GATE 1: parser do SDK
    from laya.evals import Dataset
    ds = Dataset.from_jsonl(str(out))
    print(f"GATE1 Dataset.from_jsonl: OK ({len(ds)} exemplos)")

    # GATE 2: conversão em vetores de probabilidade (contrato do records_from_labeled)
    ok = sum(numeric_targets(e["questions"], e["expected"]) is not None for e in examples)
    print(f"GATE2 numeric_targets: {ok}/{len(examples)}")
    return 0 if ok == len(examples) else 1


if __name__ == "__main__":
    raise SystemExit(main())
