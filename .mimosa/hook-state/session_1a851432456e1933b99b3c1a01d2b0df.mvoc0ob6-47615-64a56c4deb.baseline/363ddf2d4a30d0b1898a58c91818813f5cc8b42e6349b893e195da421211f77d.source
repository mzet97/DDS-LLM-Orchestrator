#!/usr/bin/env python3
"""T-900-M6-03 (piloto) — avaliação REAL do Laya no dataset semente.

Roda o dataset (32 PT/EN) pelo RealLayaAdapter (checkpoint pinado) e pelo
baseline de regras no MESMO split TEST; reporta roteamento (qos accuracy,
macro-F1 por capability, needs_planning) + latências P50/P95/P99 da decisão.

HONESTIDADE (threats): dataset SEED rotulado por engenharia (não
calibration-grade); checkpoint emite aviso de temperaturas inválidas
(confianças não calibradas — o próprio SDK avisa); amostra pequena (14
test). Números são do PILOTO, não da campanha N>=30.
"""

import json
import statistics
import sys
import time
from pathlib import Path

WORKER = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WORKER / "src"))
sys.path.insert(0, str(WORKER / "tools"))

from laya_worker.adapter import RealLayaAdapter, answer_is_self_consistent  # noqa: E402
from laya_worker import rules_baseline  # noqa: E402
from laya_worker.questions import CAPABILITY_CATALOG, QOS_PROFILES  # noqa: E402
import evaluate as _ev  # noqa: E402
DATASET, load, f1_for_capability = _ev.DATASET, _ev.load, _ev.f1_for_capability


def pct(xs, p):
    if not xs:
        return 0.0
    xs = sorted(xs)
    idx = min(len(xs) - 1, round(p / 100 * (len(xs) - 1)))
    return xs[idx]


def main() -> int:
    rows = load()
    test = [r for r in rows if r["split"] == "test"]
    adapter = RealLayaAdapter()

    lat = []
    qos_ok = 0
    plan_ok = 0
    preds_caps = []
    errors = 0
    per_row = []
    for r in test:
        t0 = time.perf_counter()
        try:
            ans = adapter.decide(r["prompt"], (time.time_ns() + 60_000_000_000))
            dt = (time.perf_counter() - t0) * 1000
            lat.append(dt)
            if not answer_is_self_consistent(ans):
                raise ValueError("inconsistente")
        except Exception as e:
            errors += 1
            per_row.append({"id": r["id"], "error": str(e)[:80]})
            continue
        qos_ok += ans.qos_profile == r["qos"]
        plan_ok += (ans.needs_planning_probability >= 0.5) == r["needs_planning"]
        caps = {c: ans.capability_probabilities.get(c, 0.0) for c in CAPABILITY_CATALOG}
        preds_caps.append(caps)
        per_row.append({
            "id": r["id"], "lang": r["lang"], "qos_pred": ans.qos_profile, "qos_gold": r["qos"],
            "caps_top": sorted(caps.items(), key=lambda x: -x[1])[:3],
            "needs_pred": round(ans.needs_planning_probability, 2), "needs_gold": r["needs_planning"],
            "confidence": round(ans.confidence, 3), "ms": round(dt),
        })

    # macro-F1 por capability (limiar 0.5) — Laya
    laya_rows = [
        {"prompt": r["prompt"], "capabilities": r["capabilities"]} for r in test
    ]
    per_cap = {}
    for cap in CAPABILITY_CATALOG:
        tp = fp = fn = 0
        for r, pred in zip(laya_rows, preds_caps, strict=False):
            p = pred[cap] >= 0.5
            g = cap in r["capabilities"]
            tp += p and g
            fp += p and not g
            fn += (not p) and g
        prec = tp / (tp + fp) if tp + fp else 0.0
        rec = tp / (tp + fn) if tp + fn else 0.0
        per_cap[cap] = round(2 * prec * rec / (prec + rec) if prec + rec else 0.0, 4)

    # baseline no MESMO split (reuso do evaluator)
    import evaluate as _ev2
    evaluate = _ev2.evaluate
    base = evaluate(load())

    report = {
        "piloto": True,
        "dataset": str(DATASET.name) + " (32 PT/EN, rótulo de engenharia)",
        "model": {"sdk": "laya==0.4.10", "checkpoint": "891102d372688fc2…", "device": "cpu"},
        "n_test": len(test),
        "errors": errors,
        "laya": {
            "qos_accuracy": round(qos_ok / max(1, len(test) - errors), 4),
            "needs_planning_accuracy": round(plan_ok / max(1, len(test) - errors), 4),
            "capability_macro_f1_at_0.5": round(sum(per_cap.values()) / len(per_cap), 4),
            "capability_f1": per_cap,
            "latency_ms": {
                "p50": round(pct(lat, 50)), "p95": round(pct(lat, 95)),
                "p99": round(pct(lat, 99)), "mean": round(statistics.mean(lat), 1) if lat else 0,
            },
            "confianca_media": round(
                statistics.mean([p["confidence"] for p in per_row if "confidence" in p]), 3
            ) if per_row else 0,
        },
        "rules_baseline": {
            "qos_accuracy": base["qos_accuracy"],
            "needs_planning_accuracy": base["needs_planning_accuracy"],
            "capability_macro_f1_at_0.5": base["capability_macro_f1_at_0.5"],
        },
        "calibration_note": "checkpoint com aviso de temperaturas inválidas (SDK): confianças NÃO calibradas; shadow segue obrigatório",
        "rows": per_row,
    }
    out = Path(sys.argv[1]) if len(sys.argv) > 1 else None
    text = json.dumps(report, indent=2, ensure_ascii=False)
    print(text[:2500])
    if out:
        out.write_text(text + "\n", encoding="utf-8")
        print(f"-> {out}", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
