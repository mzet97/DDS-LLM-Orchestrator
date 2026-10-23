#!/usr/bin/env env-python
# shebang estranho de propósito: rodar com o venv ~/.cache/tese-laya/venv/bin/python

"""T-900-A2 — calibração do checkpoint Laya (dataset por construção).

Pipeline (SDK nativo, calibrate.py/evals.py):
  1. split determinístico 80/20 (train/holdout) do dataset JSONL;
  2. records_from_labeled no TRAIN (logits CPU, sem download);
  3. fit_temperature_map (T por bucket, ECE em holdout interno);
  4. fit_abstention_thresholds + fit_binning_map;
  5. agent.save_calibration -> calibration_v1.json (payload versionado);
  6. eval ANTES/DEPOIS no HOLDOUT (accuracy por pergunta, ECE, Brier)
     via evaluate(agent, Dataset) — o agent calibrado aplica as temperaturas;
  7. relatório JSON + pin (sha256 do arquivo de calibração).

Uso: calibrate_laya.py <dataset.jsonl> <outdir> [--limit N]
"""

import argparse
import hashlib
import json
import random
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("dataset")
    ap.add_argument("outdir")
    ap.add_argument("--limit", type=int, default=0, help="usar só os N primeiros (debug)")
    ap.add_argument("--seed", type=int, default=900)
    ap.add_argument("--checkpoint", default="convaiinnovations/laya",
                    help="checkpoint dir (default: ID remoto do BASE)")
    args = ap.parse_args()

    import laya
    from laya.calibrate import (
        fit_abstention_thresholds,
        fit_binning_map,
        fit_temperature_map,
        records_from_labeled,
    )
    from laya.evals import Dataset, brier, ece, evaluate

    outdir = Path(args.outdir)
    outdir.mkdir(parents=True, exist_ok=True)

    rows = [json.loads(l) for l in Path(args.dataset).read_text().splitlines() if l.strip()]
    if args.limit:
        rows = rows[: args.limit]
    rng = random.Random(args.seed)
    rng.shuffle(rows)
    cut = int(len(rows) * 0.8)
    train, holdout = rows[:cut], rows[cut:]
    print(f"dataset: {len(rows)} | train {len(train)} | holdout {len(holdout)}", flush=True)

    agent = laya.load(args.checkpoint)

    # holdout em arquivo temporário (Dataset.from_jsonl é a porta do SDK)
    def normalize(row):
        # tags como lista de strings (formato do SDK); spec original preservida
        t = row.get("tags")
        if isinstance(t, dict):
            row = dict(row)
            row["tags"] = [f"lang:{t['spec']['lang']}", f"qos:{t['spec']['qos']}",
                           "provenance:synthetic-by-construction-gemma4"]
        return row

    holdout = [normalize(r) for r in holdout]
    train = [normalize(r) for r in train]
    hold_path = outdir / "holdout.jsonl"
    hold_path.write_text("\n".join(json.dumps(r, ensure_ascii=False) for r in holdout) + "\n")

    # ---- 1) métricas ANTES (checkpoint cru) ----
    ds_hold = Dataset.from_jsonl(str(hold_path))
    before = evaluate(agent, ds_hold)
    print("ANTES:", summary(before), flush=True)

    # ---- 2) records no TRAIN e fits ----
    # targets NUMÉRICOS (o SDK quer vetores de probabilidade): a ordem dos
    # marcadores é a ordem declarada nas perguntas (choice: critérios;
    # noul: [não, sim]; score: níveis 0..L-1).
    import sys as _sys
    _sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "tools"))
    from targets import numeric_targets

    pairs = []
    for r in train:
        t = numeric_targets(r["questions"], r["expected"])
        if t is not None:
            pairs.append((r["state"], r["questions"], t))
    print("coletando logits do train (CPU)...", flush=True)
    records = records_from_labeled(agent, pairs)
    print(f"records: {len(records)}", flush=True)

    tmap = fit_temperature_map(records, compute_ece=True)
    print("temperature map:", {k: v for k, v in tmap.items() if k in ("temperature", "ece", "n")}, flush=True)
    temp3 = tmap["temperature"]
    tbo = tmap.get("temperature_by_options") or {}
    abst = fit_abstention_thresholds(records, temp3, tbo)
    bins = fit_binning_map(records, temp3, tbo)
    from laya.calibrate import calibration_payload, apply_calibration_payload

    payload = calibration_payload(
        tmap.get("temperature"),
        tmap.get("temperature_by_options"),
        model_id_or_path="convaiinnovations/laya",
        binning_map=bins.get("binning_map") if isinstance(bins, dict) else bins,
    )
    apply_calibration_payload(agent, payload)
    calib_path = outdir / "calibration_v1.json"
    agent.save_calibration(str(calib_path))
    pin = hashlib.sha256(calib_path.read_bytes()).hexdigest()
    print(f"calibração salva: {calib_path} sha256={pin[:16]}…", flush=True)

    # thresholds de abstenção (gates) congelados junto
    gates = {"abstention": abst, "pin_sha256": pin, "train_n": len(train),
             "holdout_n": len(holdout), "records_n": len(records)}
    (outdir / "gates_v1.json").write_text(json.dumps(gates, indent=1, default=str))

    # ---- 3) métricas DEPOIS (agente calibrado) ----
    after = evaluate(agent, ds_hold)
    print("DEPOIS:", summary(after), flush=True)

    report = {
        "dataset": args.dataset,
        "n": {"total": len(rows), "train": len(train), "holdout": len(holdout)},
        "before": report_of(before),
        "after": report_of(after),
        "calibration": {"path": str(calib_path), "sha256": pin},
        "gates_path": str(outdir / "gates_v1.json"),
        "temperature_map_keys": sorted(tmap.keys()),
        "note": "dataset sintético por construção (gemma4); ruído do gerador documentado",
    }
    (outdir / "calibration_report.json").write_text(
        json.dumps(report, indent=1, ensure_ascii=False, default=str)
    )
    print(f"-> {outdir / 'calibration_report.json'}", flush=True)
    return 0


def summary(rep) -> str:
    d = report_of(rep)
    return (f"choice_acc={d.get('choice_accuracy')} noul_acc={d.get('noul_accuracy')} "
            f"score_mae={d.get('score_mae')} ece={d.get('ece')} brier={d.get('brier')}")


def report_of(rep) -> dict:
    """EvalReport -> dict plano (rep.overall do SDK)."""
    overall = getattr(rep, "overall", None)
    if isinstance(overall, dict):
        return {k: v for k, v in overall.items() if isinstance(v, (int, float))}
    return {}


if __name__ == "__main__":
    sys.exit(main())
