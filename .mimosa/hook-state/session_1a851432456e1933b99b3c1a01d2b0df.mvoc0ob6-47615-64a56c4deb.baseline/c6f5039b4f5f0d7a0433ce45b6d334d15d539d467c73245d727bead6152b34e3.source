#!/usr/bin/env python3
"""T904-M5 prep — gera (local): distill.jsonl (subconjunto p/ soft targets),
train.jsonl / train_nodistill.jsonl placeholders (join do gold vem depois), e
versões 'limpas' para upload (strip _meta em dev/cal; sealed sobe sem expected).

Uso: prepare_train_files.py --datasets <dir> --seed 904 --n-distill 1800
"""
import argparse
import hashlib
import json
import os
import random
from collections import Counter


def load(p):
    for line in open(p):
        if line.strip():
            yield json.loads(line)


def write_sdk(path, rows, keep_meta=True):
    with open(path, "w") as f:
        for i, r in enumerate(rows):
            out = {"state": r["state"], "questions": r["questions"], "expected": r["expected"]}
            if keep_meta:
                out["_meta"] = {**r.get("_meta", {}), "row_id": f"{os.path.basename(path)}:{i}"}
            f.write(json.dumps(out, ensure_ascii=False) + "\n")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--datasets", required=True)
    ap.add_argument("--seed", type=int, default=904)
    ap.add_argument("--n-distill", type=int, default=1800)
    ap.add_argument("--sealed-export", default=None,
                    help="dir do sealed pool (default: {datasets}/sealed_export)")
    args = ap.parse_args()
    d = args.datasets
    rng = random.Random(args.seed)

    train = list(load(f"{d}/train.jsonl"))
    dev = list(load(f"{d}/dev.jsonl"))
    cal = list(load(f"{d}/calibration.jsonl"))

    # distill subset: silver/silver-dual/adjudicated com rótulos completos, estratificado por qos
    cand = [r for r in train if r["_meta"]["tier"].startswith("silver")
            and "needs_planning" in r["expected"] and "qos_profile" in r["expected"]]
    by_q = {}
    for r in cand:
        by_q.setdefault(r["expected"]["qos_profile"], []).append(r)
    per = max(1, args.n_distill // max(1, len(by_q)))
    subset = []
    for q in sorted(by_q):
        rng.shuffle(by_q[q])
        subset.extend(by_q[q][:per])
    rng.shuffle(subset)
    write_sdk(f"{d}/distill.jsonl", subset)

    # train/dev/cal para upload: dev/cal sem _meta pesada (row_id preservado por ordem)
    write_sdk(f"{d}/upload_train.jsonl", train)
    write_sdk(f"{d}/upload_dev.jsonl", dev)
    write_sdk(f"{d}/upload_calibration.jsonl", cal)

    # sealed anônimo (sem expected/_meta) + labels ficam no sealed_out local
    sealed_out = args.sealed_export or os.environ.get(
        "T904_SEALED_OUT", f"{d}/sealed_export")
    os.makedirs(sealed_out, exist_ok=True)
    sealed = list(load(f"{sealed_out}/sealed_t904_pool.jsonl"))
    anon = []
    for i, r in enumerate(sealed):
        anon.append({"state": r["state"], "questions": r["questions"],
                     "_meta": {"row_id": f"sealed:{i}"}})
    with open(f"{sealed_out}/sealed_t904_anon.jsonl", "w") as f:
        for r in anon:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    print(json.dumps({
        "train": len(train), "dev": len(dev), "calibration": len(cal),
        "distill": len(subset),
        "distill_qos": dict(Counter(r["expected"]["qos_profile"] for r in subset)),
        "sealed_anon": len(anon)}, indent=1, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    import sys
    sys.exit(main())
