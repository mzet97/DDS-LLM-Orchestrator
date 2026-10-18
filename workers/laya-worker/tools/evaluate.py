#!/usr/bin/env python3
"""T-900-M2-03 — avaliação do baseline de regras no dataset semente.

Métricas (só TEST split; thresholds NÃO congelados — M2 é shadow):
- qos accuracy (acerto do perfil argmax)
- capability macro-F1 (limiar 0.5 sobre a probabilidade)
- needs_planning accuracy (limiar 0.5)
- cobertura (fração admitida; o baseline de regras não rejeita: 1.0 por
  construção — registrado honestamente)

LIMITAÇÕES (threats to validity, parte do relatório):
- Dataset SEED de 32 exemplos PT/EN rotulado pela IA executora (engenharia),
  NÃO calibration-grade; métricas servem de referência de implementação e
  fumaça, não como alegação de qualidade do Laya.
- ECE/Brier exigem probabilidades calibradas de modelo real (R-LAYA BLOCKED):
  computados apenas quando o adapter for o modelo real com splits rotulados.
"""

import json
import sys
from pathlib import Path

WORKER = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WORKER / "src"))

from laya_worker import rules_baseline  # noqa: E402
from laya_worker.questions import CAPABILITY_CATALOG, QOS_PROFILES  # noqa: E402

DATASET = WORKER / "dataset" / "dataset_v1_seed.jsonl"


def load(path: Path = DATASET) -> list[dict]:
    rows = [json.loads(l) for l in path.read_text(encoding="utf-8").splitlines() if l.strip()]
    assert rows, "dataset vazio"
    return rows


def f1_for_capability(rows: list[dict], cap: str, thr: float) -> dict:
    tp = fp = fn = 0
    for r in rows:
        pred = rules_baseline.predict(r["prompt"]).capability_probabilities.get(cap, 0.0) >= thr
        gold = cap in r["capabilities"]
        tp += pred and gold
        fp += pred and not gold
        fn += (not pred) and gold
    precision = tp / (tp + fp) if tp + fp else 0.0
    recall = tp / (tp + fn) if tp + fn else 0.0
    f1 = 2 * precision * recall / (precision + recall) if precision + recall else 0.0
    return {"precision": round(precision, 4), "recall": round(recall, 4), "f1": round(f1, 4),
            "support": tp + fn}


def evaluate(rows: list[dict]) -> dict:
    test = [r for r in rows if r["split"] == "test"]
    train = [r for r in rows if r["split"] == "train"]
    assert test and train, "splits vazios"

    qos_ok = sum(
        1 for r in test if rules_baseline.predict(r["prompt"]).qos_profile == r["qos"]
    )
    plan_ok = sum(
        1 for r in test
        if (rules_baseline.predict(r["prompt"]).needs_planning_probability >= 0.5)
        == r["needs_planning"]
    )
    per_cap = {c: f1_for_capability(test, c, 0.5) for c in CAPABILITY_CATALOG}
    macro_f1 = round(sum(v["f1"] for v in per_cap.values()) / len(per_cap), 4)

    return {
        "dataset": "dataset_v1_seed (32 exemplos PT/EN, rótulo de engenharia — NÃO calibration-grade)",
        "model": rules_baseline.MODEL_ID,
        "splits": {"train": len(train), "test": len(test)},
        "qos_accuracy": round(qos_ok / len(test), 4),
        "needs_planning_accuracy": round(plan_ok / len(test), 4),
        "capability_macro_f1_at_0.5": macro_f1,
        "capability_per_class": per_cap,
        "coverage": 1.0,
        "coverage_note": "regras não rejeitam; cobertura 1.0 por construção",
        "thresholds_frozen": False,
        "ece_brier": "PENDENTE: exigem modelo real calibrado (R-LAYA BLOCKED)",
        "langs": sorted({r["lang"] for r in rows}),
    }


def main() -> int:
    report = evaluate(load())
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else None
    text = json.dumps(report, indent=2, ensure_ascii=False)
    print(text)
    if out:
        out.write_text(text + "\n", encoding="utf-8")
        print(f"-> {out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
