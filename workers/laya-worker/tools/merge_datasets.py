#!/usr/bin/env python3
"""T-900-A3 — merge multi-ambiente dos datasets de calibração.

Une: dataset local (791) + chunks do Colab (partial-*.jsonl/chunk-*.jsonl) →
dedup por texto normalizado → shuffle seed 900 → split 80/20 ÚNICO pós-união
(sem vazamento de holdout entre ambientes) → dataset_calib_v2.jsonl + holdout.

Uso: merge_datasets.py <out_dir> <input1.jsonl> [input2 ...] [--seed 900]
"""

import argparse
import json
import random
import re
import sys
from pathlib import Path


def norm(t: str) -> str:
    return re.sub(r"\s+", " ", t.lower()).strip()[:120]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("outdir")
    ap.add_argument("inputs", nargs="+")
    ap.add_argument("--seed", type=int, default=900)
    args = ap.parse_args()

    seen = set()
    rows = []
    per_input = []
    for path in args.inputs:
        n_in = n_ok = 0
        for line in Path(path).read_text().splitlines():
            if not line.strip():
                continue
            n_in += 1
            r = json.loads(line)
            key = norm(r["state"]["text"])
            if key in seen:
                continue
            seen.add(key)
            rows.append(r)
            n_ok += 1
        per_input.append((path, n_in, n_ok))

    rng = random.Random(args.seed)
    rng.shuffle(rows)
    cut = int(len(rows) * 0.8)
    train, holdout = rows[:cut], rows[cut:]

    out = Path(args.outdir)
    out.mkdir(parents=True, exist_ok=True)
    (out / "dataset_calib_v2.jsonl").write_text(
        "\n".join(json.dumps(r, ensure_ascii=False) for r in rows) + "\n")
    (out / "train.jsonl").write_text(
        "\n".join(json.dumps(r, ensure_ascii=False) for r in train) + "\n")
    (out / "holdout.jsonl").write_text(
        "\n".join(json.dumps(r, ensure_ascii=False) for r in holdout) + "\n")

    print(f"inputs: {[(Path(p).name, i, ok) for p, i, ok in per_input]}")
    print(f"total único: {len(rows)} | train: {len(train)} | holdout: {len(holdout)}")
    # stats por bucket (floors do SDK: 2000/bucket)
    from collections import Counter
    qtype = Counter()
    for r in train:
        for q in r["questions"].values():
            qtype[q.get("type", "?")] += 1
    print(f"records por qtype (train): {dict(qtype)}")
    print("floors: noul precisa ~9x/exemplo; choice/score 1x — ver calibração")
    return 0


if __name__ == "__main__":
    sys.exit(main())
