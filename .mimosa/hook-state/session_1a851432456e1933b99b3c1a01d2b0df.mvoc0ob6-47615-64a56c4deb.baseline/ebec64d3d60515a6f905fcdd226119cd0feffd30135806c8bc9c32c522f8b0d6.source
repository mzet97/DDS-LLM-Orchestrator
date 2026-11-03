#!/usr/bin/env python3
"""VM-side T904: avaliação por qid/classe de um checkpoint sobre um split.

Env: T904_MODEL (dir), T904_SPLIT (path jsonl), T904_OUT (json de saída),
     T904_CALIB (opcional: calibration json a carregar no agent).
Métricas por qid + confusão 5x5 de qos + STREAM P/R/F1 + FPR BAL→STREAM +
latência p50/p95. Um único forward por linha (sem ensemble).
"""
import json
import math
import os
import time

ROOT = "/content/laya_t905"
model_dir = os.environ["T904_MODEL"]
split = os.environ["T904_SPLIT"]
out_path = os.environ["T904_OUT"]
calib = os.environ.get("T904_CALIB", "")

import sys  # noqa: E402
sys.path.insert(0, f"{ROOT}/tools")
import torch  # noqa: E402
import laya  # noqa: E402
from questions import CAPABILITY_CATALOG, QUESTIONS_V1  # noqa: E402

Q = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
for c in CAPABILITY_CATALOG:
    Q[f"cap__{c}"] = {"type": "noul",
                      "instructions": f"Does this request require the capability '{c}'?"}
PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
CAPS = CAPABILITY_CATALOG

kwargs = {}
if calib and os.path.exists(calib):
    kwargs["calibration"] = calib
agent = laya.load(model_dir, **kwargs)

rows = [json.loads(l) for l in open(split) if l.strip()]
labels = {}
if os.path.exists(split + ".labels.json"):
    raw_labels = json.load(open(split + ".labels.json"))
    labels = {i: (l["expected"] if isinstance(l, dict) and "expected" in l else l)
              for i, l in enumerate(raw_labels)}

conf = [[0] * 5 for _ in range(5)]
per_q = {"qos_profile": [0, 0], "needs_planning": [0, 0]}
stream = {"tp": 0, "fp": 0, "fn": 0, "tn": 0}
cap_f1 = {c: [0, 0, 0, 0] for c in CAPS}
cxs, ugs = [], []
u_conf = [[0]*3 for _ in range(3)]
c_conf = [[0]*4 for _ in range(4)]
lat = []
t0 = time.time()
for i, r in enumerate(rows):
    exp = labels.get(i, r.get("expected", {}))
    ts = time.time()
    ans = agent.system_one(r["state"], Q)["answers"]
    lat.append(time.time() - ts)
    a = ans["qos_profile"]
    pred = a.get("choice")
    gold = exp["qos_profile"]
    conf[PROFILES.index(gold)][PROFILES.index(pred)] += 1
    per_q["qos_profile"][0] += pred == gold
    per_q["qos_profile"][1] += 1
    if gold == "QI_STREAM_LIKE":
        stream["tp" if pred == gold else "fn"] += 1
    elif pred == "QI_STREAM_LIKE":
        stream["fp"] += 1
    else:
        stream["tn"] += 1
    pn = float(ans["needs_planning"].get("noul", 0.0)) >= 0.5
    per_q["needs_planning"][0] += pn == bool(exp["needs_planning"])
    per_q["needs_planning"][1] += 1
    cs = ans["complexity"]
    cxs.append((sum(int(k) * v for k, v in cs.get("probabilities", {}).items()) or float(cs.get("score", 0)),
                exp["complexity"]))
    us = ans["urgency"]
    ugs.append((sum(int(k) * v for k, v in us.get("probabilities", {}).items()) or float(us.get("score", 0)),
                exp["urgency"]))
    u_pr = us.get("probabilities") or {}
    u_am = int(max(u_pr, key=u_pr.get)) if u_pr else int(float(us.get("score", 0)))
    u_conf[int(exp["urgency"])][u_am] += 1
    c_pr = cs.get("probabilities") or {}
    c_am = int(max(c_pr, key=c_pr.get)) if c_pr else int(float(cs.get("score", 0)))
    c_conf[int(exp["complexity"])][c_am] += 1
    for c in CAPS:
        pc = float(ans[f"cap__{c}"].get("noul", 0.0)) >= 0.5
        gc = bool(exp.get(f"cap__{c}", False))
        cap_f1[c][0 if pc and gc else 1 if pc else 2 if gc else 3] += 1  # tp/fp/fn/tn idx hack
# F1 por cap a partir dos contadores (tp, fp+fn por slots)
caps_metrics = {}
for c in CAPS:
    tp = cap_f1[c][0]
    fp = cap_f1[c][1]
    fn = cap_f1[c][2]
    prec = tp / (tp + fp) if tp + fp else 0.0
    rec = tp / (tp + fn) if tp + fn else 0.0
    caps_metrics[c] = {"precision": round(prec, 4), "recall": round(rec, 4),
                       "f1": round(2 * prec * rec / (prec + rec), 4) if prec + rec else 0.0}

def _macro_f1(conf):
    k = len(conf)
    f1s = []
    for i in range(k):
        tp = conf[i][i]
        fp = sum(conf[j][i] for j in range(k)) - tp
        fn = sum(conf[i]) - tp
        p = tp / (tp + fp) if tp + fp else 0.0
        r = tp / (tp + fn) if tp + fn else 0.0
        f1s.append(2 * p * r / (p + r) if p + r else 0.0)
    return round(sum(f1s) / k, 4)


lat.sort()


def pct(p):
    return round(lat[min(len(lat) - 1, int(len(lat) * p))] * 1000, 1)


cm_metrics = {}
for i, g in enumerate(PROFILES):
    tp = conf[i][i]
    fp = sum(conf[j][i] for j in range(5)) - tp
    fn = sum(conf[i]) - tp
    prec = tp / (tp + fp) if tp + fp else 0.0
    rec = tp / (tp + fn) if tp + fn else 0.0
    cm_metrics[g] = {"precision": round(prec, 4), "recall": round(rec, 4),
                     "f1": round(2 * prec * rec / (prec + rec), 4) if prec + rec else 0.0,
                     "support": sum(conf[i])}
macro_f1 = sum(v["f1"] for v in cm_metrics.values()) / 5
stream_p = stream["tp"] / (stream["tp"] + stream["fp"]) if stream["tp"] + stream["fp"] else 0.0
stream_r = stream["tp"] / (stream["tp"] + stream["fn"]) if stream["tp"] + stream["fn"] else 0.0
report = {
    "model": model_dir, "split": split, "n": len(rows),
    "qos": {"confusion_matrix": dict(zip(PROFILES,
            [dict(zip(PROFILES, row)) for row in conf])),
            "per_class": cm_metrics, "macro_f1": round(macro_f1, 4),
            "accuracy": round(per_q["qos_profile"][0] / max(1, per_q["qos_profile"][1]), 4)},
    "stream": {"precision": round(stream_p, 4), "recall": round(stream_r, 4),
               "f1": round(2 * stream_p * stream_r / (stream_p + stream_r), 4) if stream_p + stream_r else 0.0,
               "fpr_balanced_to_stream": round(stream["fp"] / max(1, stream["fp"] + stream["tn"]), 4)},
    "needs_planning": {"accuracy": round(per_q["needs_planning"][0] / max(1, per_q["needs_planning"][1]), 4)},
    "complexity": {"macro_f1": _macro_f1(c_conf), "confusion": c_conf,
                "mae": round(sum(abs(a - g) for a, g in cxs) / max(1, len(cxs)), 4),
                   "within_one": round(sum(1 for a, g in cxs if abs(a - g) <= 1) / max(1, len(cxs)), 4)},
    "urgency": {"macro_f1": _macro_f1(u_conf), "confusion": u_conf,
                "mae": round(sum(abs(a - g) for a, g in ugs) / max(1, len(ugs)), 4),
                "within_one": round(sum(1 for a, g in ugs if abs(a - g) <= 1) / max(1, len(ugs)), 4)},
    "caps_macro_f1_mean": round(sum(v["f1"] for v in caps_metrics.values()) / len(CAPS), 4),
    "caps": caps_metrics,
    "latency_ms": {"p50": pct(0.50), "p95": pct(0.95), "p99": pct(0.99)},
    "total_s": round(time.time() - t0, 1),
}
os.makedirs(os.path.dirname(out_path) or ".", exist_ok=True)
json.dump(report, open(out_path, "w"), indent=1, ensure_ascii=False)
print("EVAL_DONE", out_path, flush=True)
print(json.dumps({k: report[k] for k in ("qos", "stream", "needs_planning", "caps_macro_f1_mean")}, indent=1)[:1500], flush=True)
