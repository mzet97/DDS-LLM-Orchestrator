#!/usr/bin/env python3
"""VM-side T904-M6: calibração por temperatura do finalista no split CALIBRATION.
Env: T904_MODEL, T904_SPLIT, T904_OUTDIR. Grava calibration_v1.json + gates.
"""
import json
import os
import sys

ROOT = "/content/laya_t905"
model_dir = os.environ["T904_MODEL"]
split = os.environ["T904_SPLIT"]
outdir = os.environ["T904_OUTDIR"]

sys.path.insert(0, f"{ROOT}/tools")
import laya  # noqa: E402
from laya.calibrate import fit_abstention_thresholds, fit_temperature_map, records_from_labeled  # noqa: E402
from targets import numeric_targets  # módulo do projeto (workers/laya-worker/tools)
from questions import CAPABILITY_CATALOG, QUESTIONS_V1  # noqa: E402

Q = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
for c in CAPABILITY_CATALOG:
    Q[f"cap__{c}"] = {"type": "noul",
                      "instructions": f"Does this request require the capability '{c}'?"}

agent = laya.load(model_dir)
rows = [json.loads(l) for l in open(split) if l.strip()]
pairs = []
for r in rows:
    tg = numeric_targets(Q, r["expected"])
    if tg:
        pairs.append((r["state"], Q, tg))
print(f"calibration pairs: {len(pairs)}", flush=True)
from laya.calibrate import apply_calibration_payload  # noqa: E402
records = records_from_labeled(agent, pairs)
tmap = fit_temperature_map(records, seed=904)
apply_calibration_payload(agent, tmap)
os.makedirs(outdir, exist_ok=True)
out = f"{outdir}/calibration_v1.json"
agent.save_calibration(out)
import hashlib  # noqa: E402
pin = hashlib.sha256(open(out, "rb").read()).hexdigest()
print("CALIB_DONE pin=", pin, flush=True)
print(json.dumps({"pin": pin, "path": out, "temperature_map": tmap.get("temperature"),
                  "n_pairs": len(pairs)}), flush=True)
