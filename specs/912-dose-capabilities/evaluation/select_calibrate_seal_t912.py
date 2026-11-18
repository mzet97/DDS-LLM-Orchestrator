#!/usr/bin/env python3
"""T912-F4b/F5 — Seleção finalista + calibração CAL + abertura única sealed + pareado.

PORTÁTIL (RX/.61/SDumont): paths via T912_ROOT/T912_REPO/T912_WORK (defaults relativos).
Rodar APÓS o treino dos braços com dev evals prontos. Um comando, logs em results/infra/.

Uso: python select_calibrate_seal_t912.py [--skip-sealed]
  --skip-sealed: para após seleção+calibração (não abre o sealed; permite inspeção)
"""
import argparse
import json
import os
import subprocess
import sys
import time

HERE = os.path.dirname(os.path.abspath(__file__))
SPEC = os.environ.get("T912_ROOT") or os.path.normpath(os.path.join(HERE, ".."))
REPO = os.environ.get("T912_REPO") or os.path.normpath(os.path.join(SPEC, "..", ".."))
sys.path.insert(0, os.path.join(REPO, "workers", "laya-worker", "src"))

DATA = os.path.join(SPEC, "datasets")
SEALED = os.path.join(SPEC, "sealed")
RESULTS = os.environ.get("T912_OUT") or os.path.join(SPEC, "results")
WORK = os.environ.get("T912_WORK") or os.path.expanduser("~/.cache/tese-laya/t912_train")
PY = sys.executable
LOGDIR = os.path.join(RESULTS, "infra")
LOG = os.path.join(LOGDIR, "select_calib_seal.log")


def log(m):
    line = f"[{time.strftime('%H:%M:%S')}] {m}"
    os.makedirs(LOGDIR, exist_ok=True)
    with open(LOG, "a") as f:
        f.write(line + "\n")
    print(line, flush=True)


def run(cmd, logfile):
    with open(logfile, "a") as f:
        r = subprocess.run(cmd, stdout=f, stderr=subprocess.STDOUT)
    return r.returncode


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--skip-sealed", action="store_true")
    a = ap.parse_args()
    os.makedirs(RESULTS, exist_ok=True)

    # 1. seleção por min-F1 no dev (empate -> caps_pos_macro -> custo C1)
    arms = {}
    for arm in ("C0", "C1"):
        p = os.path.join(RESULTS, f"{arm}_dev", "metrics.json")
        if os.path.exists(p):
            arms[arm] = json.load(open(p))
    if not arms:
        log("ERRO: nenhum dev metrics")
        sys.exit(1)
    for arm, m in arms.items():
        log(f"{arm} dev: minF1={m['min_f1_all_classes']} qos={m['qos']['macro_f1']} "
            f"cx={m['complexity']['macro_f1']} urg={m['urgency']['macro_f1']} "
            f"plan={m['planning']['acc']} caps={m['caps']['pos_macro_f1']}")
    order = {"C1": 0, "C0": 1}  # custo: prefere C1 (0.8B) em empates exatos
    best = max(arms, key=lambda x: (arms[x]["min_f1_all_classes"],
                                    arms[x]["caps"]["pos_macro_f1"], order[x]))
    log(f"FINALISTA: {best}")

    # 2. calibração do finalista em CAL (build_dataset antes; bug KeyError 'record' T911)
    from unsloth import FastDecisionModel
    from datasets import Dataset
    from laya_worker.questions import QUESTIONS_V1, CAPABILITY_CATALOG
    Q12 = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
    for c in CAPABILITY_CATALOG:
        Q12[f"cap__{c}"] = {"type": "noul",
                            "instructions": f"Does this request require the capability '{c}'?"}
    src = os.path.join(WORK, f"arm_{best}")
    dst = os.path.join(WORK, f"arm_{best}_cal")
    model, tokenizer = FastDecisionModel.from_pretrained(model_name=src, max_seq_length=2048)
    rows = [json.loads(l) for l in open(os.path.join(DATA, "cal_t912.jsonl")) if l.strip()]
    for r in rows:
        r["questions"] = Q12
    items, _ = FastDecisionModel.build_dataset(Dataset.from_list(rows), tokenizer, model)
    log(f"CAL build_dataset: {len(items)} itens")
    rep = FastDecisionModel.calibrate(model, tokenizer, items)
    log(f"calibrate: {json.dumps(rep, default=str)[:200]}")
    model.save_pretrained(dst)
    tokenizer.save_pretrained(dst)
    cal_dir = os.path.join(RESULTS, "calibration")
    os.makedirs(cal_dir, exist_ok=True)
    json.dump({"source": src, "calibrated": dst, "cal_rows": len(rows), "report": rep},
              open(os.path.join(cal_dir, "CALIBRATION_T912.json"), "w"), indent=1, default=str)
    json.dump({"arm": best, "dev_metrics": arms[best]},
              open(os.path.join(RESULTS, "finalista.json"), "w"), indent=1)
    if a.skip_sealed:
        log("T912_SELECT_CALIB_DONE (--skip-sealed; sealed NAO aberto)")
        return

    # 3. abertura ÚNICA do sealed + pareado C0/C1
    proto_p = os.path.join(SEALED, "SEALED_OPEN_PROTOCOL.json")
    proto = json.load(open(proto_p))
    assert proto["status"] == "NOT_OPENED" and proto["open_count"] == 0, proto
    proto.update({"status": "OPENED", "open_count": 1,
                  "opened_at": time.strftime("%Y-%m-%dT%H:%M:%S"),
                  "frozen_before_open": {
                      "finalista": best,
                      "calibrador": f"{dst} (calibration/CALIBRATION_T912.json)",
                      "arms_paired": ["C0", "C1"],
                      "prereg": "specs/912-dose-capabilities/PREREG_T912.json"}})
    json.dump(proto, open(proto_p, "w"), indent=1)
    log(f"SEALED ABERTO (open_count=1) n={proto['n']}")

    for arm in ("C0", "C1"):
        d = os.path.join(WORK, f"arm_{arm}")
        if not os.path.exists(os.path.join(d, "adapter_model.safetensors")):
            log(f"SKIP {arm} (sem adapter)")
            continue
        rc = run([PY, os.path.join(HERE, "eval_t912.py"), "--arm-dir", d,
                  "--split", os.path.join(SEALED, "pool.jsonl"),
                  "--labels", os.path.join(SEALED, "labels.json"),
                  "--out", os.path.join(RESULTS, f"{arm}_sealed"), "--bootstrap", "1000"],
                 os.path.join(LOGDIR, f"eval_{arm}_sealed.log"))
        log(f"sealed eval {arm} rc={rc}")

    # 4. gates
    gates_tmp = os.path.join(RESULTS, "gates_tmp")
    os.makedirs(gates_tmp, exist_ok=True)
    for arm in ("C0", "C1"):
        d = os.path.join(gates_tmp, f"{arm}_sealed")
        os.makedirs(d, exist_ok=True)
        src_m = os.path.join(RESULTS, f"{arm}_sealed", "metrics.json")
        if os.path.exists(src_m):
            run(["cp", src_m, os.path.join(d, "metrics.json")], "/dev/null")
    rc = run([PY, os.path.join(HERE, "gates_t912.py"),
              "--eval-root", gates_tmp,
              "--out", os.path.join(RESULTS, "GATES_FINAL_T912.json")],
             os.path.join(LOGDIR, "gates.log"))
    log(f"gates rc={rc} -> {os.path.join(RESULTS, 'GATES_FINAL_T912.json')}")
    log("T912_SELECT_CALIB_SEAL_DONE")


if __name__ == "__main__":
    main()
