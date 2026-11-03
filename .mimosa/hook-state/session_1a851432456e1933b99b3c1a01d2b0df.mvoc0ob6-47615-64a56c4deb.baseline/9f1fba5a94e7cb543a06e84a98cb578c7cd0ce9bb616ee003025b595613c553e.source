#!/usr/bin/env python3
"""Calibração v4 do Laya no Google Colab (GPU) — dataset SPROUT derivado.

Autocontido: baixa o SPROUT (HF), converte com rótulos POR MEDIÇÃO (Pareto
custo×qualidade), coleta logits na GPU (chunks com flush), calibra
(temperature/abstention/binning) e devolve métricas ANTES/DEPOIS no split
de TESTE oficial do SPROUT. Marcas CHUNK_<FASE>_BEGIN/END no stdout.

Driver local: tools/colab_calib/ (ver README na evidência calib_v4).
"""
import hashlib
import json
import pickle
import random
import shutil
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

T0 = time.time()
OUT = Path("/content/out")  # criado em main() (permite import local p/ testes)

QOS_DESC = {
    "QI_BALANCED": "um trabalho comum, sem restrições especiais",
    "QI_CRITICAL": "um trabalho crítico, urgente e sensível a prazo (produção)",
    "QI_STREAM_LIKE": "um trabalho interativo com resposta em streaming",
    "QI_LOW_COST": "um trabalho em lote, barato, sem urgência",
    "QI_FAILOVER": "um trabalho que exige resiliência a falhas de infraestrutura",
}
QOS_PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
CAPABILITY_CATALOG = [
    "rust_refactor", "python_refactor", "unit_testing", "security_audit",
    "code_review", "research", "summarization", "translation",
]

FRONTIER = ["openai-gpt-4o", "aws-claude-3-5-sonnet-v1",
            "wxai-llama-3-405b-instruct", "wxai-llama-3-3-70b-instruct"]
MID = ["wxai-llama-3-1-70b-instruct"]
CHEAP = ["openai-gpt-4o-mini", "wxai-llama-3-2-1b-instruct",
         "wxai-llama-3-2-3b-instruct", "wxai-llama-3-1-8b-instruct",
         "wxai-granite-3-2b-instruct-8k-max-tokens",
         "wxai-granite-3-8b-instruct-8k-max-tokens",
         "wxai-mixtral-8x7b-instruct-v01", "aws-titan-text-premier-v1"]


def mark(msg: str):
    print(f"[{time.time() - T0:7.1f}s] {msg}", flush=True)


def chunk_end(label: str, **kv):
    print(f"CHUNK_{label}_END {json.dumps(kv, ensure_ascii=False, default=str)}", flush=True)


def questions_for() -> dict:
    q = {
        "qos_profile": {"type": "choice",
                        "instructions": "Which QoS objective best fits this workload?",
                        "criteria": {p: QOS_DESC[p] for p in QOS_PROFILES}},
        "needs_planning": {"type": "noul",
                           "instructions": "Does the request require multiple dependent tasks?"},
        "complexity": {"type": "score", "instructions": "How complex is the task?",
                       "criteria": ["simple", "moderate", "complex", "very_complex"]},
        "urgency": {"type": "score", "instructions": "How urgent is the task?",
                    "criteria": ["low", "normal", "high"]},
    }
    for cap in CAPABILITY_CATALOG:
        q[f"cap__{cap}"] = {"type": "noul",
                            "instructions": f"Does this request require the capability '{cap}'?"}
    return q


def scores(row: dict, models) -> list:
    out = []
    for m in models:
        v = row.get(m) or {}
        s = v.get("score") if isinstance(v, dict) else None
        if isinstance(s, (int, float)):
            out.append(float(s))
    return out


def label_qos(row: dict) -> str:
    cheap_max = max(scores(row, CHEAP) or [0.0])
    mid_max = max(scores(row, MID) or [0.0])
    front_max = max(scores(row, FRONTIER) or [0.0])
    if cheap_max >= 0.95:
        return "QI_LOW_COST"
    if front_max >= 0.90 and front_max - cheap_max >= 0.25:
        return "QI_CRITICAL"
    if mid_max >= 0.90:
        return "QI_BALANCED"
    return "QI_BALANCED"


def label_complexity(row: dict) -> int:
    allsc = scores(row, FRONTIER + MID + CHEAP) or [0.0]
    mean, mx = sum(allsc) / len(allsc), max(allsc)
    if mx < 0.5:
        return 3
    if mean >= 0.85:
        return 0
    if mean >= 0.70:
        return 1
    return 2


def convert(row: dict) -> dict:
    src = row.get("dataset", "")
    planning = src.startswith("lighteval/MATH")
    caps = {f"cap__{c}": False for c in CAPABILITY_CATALOG}
    caps["cap__research"] = "ragbench" in src
    qos = label_qos(row)
    return {
        "state": {"text": row["prompt"]},
        "questions": questions_for(),
        "expected": {"qos_profile": qos, "needs_planning": planning,
                     "complexity": label_complexity(row), "urgency": 1, **caps},
        "language": "en",
        "tags": ["lang:en", f"qos:{qos}", f"src:{src}",
                 f"planning:{int(planning)}", "provenance:sprout-pareto-measured"],
    }


def one_hot(n, idx):
    v = [0.0] * n
    v[idx] = 1.0
    return v


def numeric_targets(questions: dict, expected: dict):
    out = {}
    for qid, q in questions.items():
        t, e = q.get("type"), expected.get(qid)
        if t == "choice":
            keys = list(q.get("criteria", {}).keys())
            if e not in keys:
                return None
            out[qid] = one_hot(len(keys), keys.index(e))
        elif t == "noul":
            p = float(bool(e))
            out[qid] = [1.0 - p, p]
        elif t == "score":
            levels = q.get("criteria") or [str(i) for i in range(4)]
            idx = int(e) if isinstance(e, (int, float)) else levels.index(e)
            if not (0 <= idx < len(levels)):
                return None
            out[qid] = one_hot(len(levels), idx)
        else:
            return None
    return out if set(out) == set(questions) else None


def report_of(rep) -> dict:
    overall = getattr(rep, "overall", None)
    if isinstance(overall, dict):
        return {k: v for k, v in overall.items() if isinstance(v, (int, float))}
    return {}


def summary(d: dict) -> str:
    return (f"choice_acc={d.get('choice_accuracy')} noul_acc={d.get('noul_accuracy')} "
            f"score_mae={d.get('score_mae')} ece={d.get('ece')} brier={d.get('brier')}")


def write_jsonl(path: Path, rows):
    path.write_text("\n".join(json.dumps(r, ensure_ascii=False) for r in rows) + "\n")


def main() -> int:
    OUT.mkdir(parents=True, exist_ok=True)
    mark("fase 0: instalando laya==0.4.10 (torch do Colab permanece)")
    subprocess.run([sys.executable, "-m", "pip", "install", "-q", "laya==0.4.10"],
                   check=True)

    # ---- fase 1: SPROUT ----
    print("CHUNK_DATA_BEGIN", flush=True)
    mark("fase 1: baixando SPROUT")
    from datasets import load_dataset
    ds = load_dataset("CARROT-LLM-Routing/SPROUT")
    rng = random.Random(900)
    train_rows = list(ds["train"])
    test_rows = list(ds["test"])
    rng.shuffle(train_rows)
    rng.shuffle(test_rows)
    train = [convert(r) for r in train_rows[:12000]]
    test = [convert(r) for r in test_rows[:2000]]
    write_jsonl(OUT / "v4_train.jsonl", train)
    write_jsonl(OUT / "v4_test.jsonl", test)
    from collections import Counter
    dist = {
        "train_qos": dict(Counter(e["expected"]["qos_profile"] for e in train)),
        "test_qos": dict(Counter(e["expected"]["qos_profile"] for e in test)),
        "train_complexity": dict(Counter(e["expected"]["complexity"] for e in train)),
        "train_planning": dict(Counter(str(e["expected"]["needs_planning"]) for e in train)),
    }
    mark(f"distribuição: {json.dumps(dist)}")
    chunk_end("DATA", train_n=len(train), test_n=len(test), **dist)

    # ---- fase 2: agente GPU + ANTES ----
    print("CHUNK_ANTES_BEGIN", flush=True)
    mark("fase 2: carregando laya em cuda + métricas ANTES (teste real)")
    import laya
    from laya.evals import Dataset, evaluate
    agent = laya.load("convaiinnovations/laya", device="cuda")
    ds_test = Dataset.from_jsonl(str(OUT / "v4_test.jsonl"))
    before = evaluate(agent, ds_test)
    d_before = report_of(before)
    mark("ANTES: " + summary(d_before))
    chunk_end("ANTES", **d_before)

    # ---- fase 3: records na GPU (chunks com flush) ----
    print("CHUNK_RECORDS_BEGIN", flush=True)
    mark("fase 3: coletando logits do train (cuda)")
    from laya.calibrate import (records_from_labeled, fit_temperature_map,
                                fit_abstention_thresholds, fit_binning_map,
                                calibration_payload, apply_calibration_payload)
    pairs = []
    for r in train:
        t = numeric_targets(r["questions"], r["expected"])
        if t is not None:
            pairs.append((r["state"], r["questions"], t))
    records, CH = [], 1500
    for i in range(0, len(pairs), CH):
        t_ch = time.time()
        records.extend(records_from_labeled(agent, pairs[i:i + CH]))
        with open(OUT / "records_partial.pkl", "wb") as f:
            pickle.dump(records, f)
        rate = CH / (time.time() - t_ch)
        eta = (len(pairs) - min(i + CH, len(pairs))) / max(rate, 0.01)
        mark(f"records {min(i + CH, len(pairs))}/{len(pairs)} "
             f"({rate:.1f} ex/s, ETA {eta / 60:.1f} min)")
    chunk_end("RECORDS", records_n=len(records), pairs_n=len(pairs))

    # ---- fase 4: fits + payload ----
    print("CHUNK_FITS_BEGIN", flush=True)
    mark("fase 4: fits (temperature/abstention/binning)")
    tmap = fit_temperature_map(records, compute_ece=True)
    temp3 = tmap["temperature"]
    tbo = tmap.get("temperature_by_options") or {}
    abst = fit_abstention_thresholds(records, temp3, tbo)
    bins = fit_binning_map(records, temp3, tbo)
    payload = calibration_payload(
        tmap.get("temperature"), tmap.get("temperature_by_options"),
        model_id_or_path="convaiinnovations/laya",
        binning_map=bins.get("binning_map") if isinstance(bins, dict) else bins,
    )
    apply_calibration_payload(agent, payload)
    calib_path = OUT / "calibration_v4.json"
    agent.save_calibration(str(calib_path))
    pin = hashlib.sha256(calib_path.read_bytes()).hexdigest()
    mark(f"calibração v4 salva sha256={pin[:16]}…")
    gates = {"abstention": abst, "pin_sha256": pin, "train_n": len(train),
             "holdout_n": len(test), "records_n": len(records)}
    (OUT / "gates_v4.json").write_text(json.dumps(gates, indent=1, default=str))
    chunk_end("FITS", pin=pin, temperature=temp3,
              temperature_by_options=tbo, abstention=abst)

    # ---- fase 5: DEPOIS ----
    print("CHUNK_DEPOIS_BEGIN", flush=True)
    mark("fase 5: métricas DEPOIS no mesmo teste real")
    after = evaluate(agent, ds_test)
    d_after = report_of(after)
    mark("DEPOIS: " + summary(d_after))
    chunk_end("DEPOIS", **d_after)

    report = {
        "dataset": "CARROT-LLM-Routing/SPROUT (derivado sprout_to_laya, seed 900)",
        "n": {"train": len(train), "holdout": len(test), "records": len(records)},
        "labeling": "POR MEDIÇÃO: Pareto custo×qualidade sobre scores de juiz "
                    "de 13 modelos (cheap>=0.95→LOW_COST; frontier>=0.9 com gap"
                    ">=0.25→CRITICAL; mid>=0.9→BALANCED); complexidade por "
                    "dificuldade medida; urgency=1 neutro; caps code=False",
        "before": d_before, "after": d_after,
        "calibration": {"path": "calibration_v4.json", "sha256": pin},
        "temperature_map": {"temperature": temp3, "temperature_by_options": tbo},
        "abstention": abst,
        "distribution": dist,
        "note": "holdout = split TEST oficial do SPROUT (dados reais, nunca "
                "vistos no fit); license do SPROUT sem campo explícito",
    }
    (OUT / "calibration_report_v4.json").write_text(
        json.dumps(report, indent=1, ensure_ascii=False, default=str))
    (OUT / "records_partial.pkl").unlink(missing_ok=True)  # pickle grande: não baixar
    files = sorted(p.name for p in OUT.iterdir())
    mark(f"concluído: {files}")
    print(f"CHUNK_ALL_END {json.dumps({'files': files, 'pin': pin})}", flush=True)
    return 0


if __name__ == "__main__":
    raise SystemExit(main())
