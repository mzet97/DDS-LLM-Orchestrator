#!/usr/bin/env python3
"""T904-M3 — splits agrupados por família + sealed T904 congelado.

Regras: reused-* e template-hardneg vão TODOS para train (proteção por construção);
famílias new-gen são divididas por hash+seed em train-rest / dev / calibration /
sealed-t904. Sealed: famílias inteiras, 5 classes QoS representadas (greedy
balanceado), casos com e sem literal "stream". Manifest congelado com SHAs e
contagens por classe/qid ANTES de qualquer treino; sealed sai da árvore de
trabalho (cópia com chmod 000 e hash registrado).
"""
import argparse
import hashlib
import json
import os
import random
import shutil
from collections import Counter, defaultdict


def load(p):
    with open(p) as f:
        for line in f:
            if line.strip():
                yield json.loads(line)


def fam_sort_key(f):
    return hashlib.sha1(f.encode()).hexdigest()


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--pool", required=True)
    ap.add_argument("--outdir", required=True)
    ap.add_argument("--sealed-out", required=True, help="dir FORA da árvore de treino")
    ap.add_argument("--seed", type=int, default=904)
    ap.add_argument("--sealed-frac", type=float, default=0.16)
    ap.add_argument("--dev-frac", type=float, default=0.10)
    ap.add_argument("--cal-frac", type=float, default=0.07)
    args = ap.parse_args()
    rng = random.Random(args.seed)
    os.makedirs(args.outdir, exist_ok=True)
    os.makedirs(args.sealed_out, exist_ok=True)

    rows = list(load(args.pool))
    by_fam = defaultdict(list)
    for r in rows:
        by_fam[r["_meta"]["family"]].append(r)

    new_fams = sorted([f for f, rs in by_fam.items()
                       if rs[0]["_meta"]["partition_hint"] == "new-gen"
                       and rs[0]["_meta"]["tier"] != "review-leak-suspect"], key=fam_sort_key)
    rng.shuffle(new_fams)
    n = len(new_fam_sets := new_fams)
    n_sealed = max(8, round(n * args.sealed_frac))
    n_dev = max(6, round(n * args.dev_frac))
    n_cal = max(4, round(n * args.cal_frac))
    sealed_fams = set(new_fams[:n_sealed])
    dev_fams = set(new_fams[n_sealed:n_sealed + n_dev])
    cal_fams = set(new_fams[n_sealed + n_dev:n_sealed + n_dev + n_cal])

    buckets = {"train": [], "dev": [], "calibration": [], "sealed": []}
    for f, rs in by_fam.items():
        if f in sealed_fams:
            buckets["sealed"].extend(rs)
        elif f in dev_fams:
            buckets["dev"].extend(rs)
        elif f in cal_fams:
            buckets["calibration"].extend(rs)
        else:
            buckets["train"].extend(rs)

    # greedy de balanceamento do sealed: se faltar QI_STREAM_LIKE/QI_FAILOVER,
    # troca famílias train↔sealed (inteiras, sem quebrar agrupamento)
    def qos_of(r):
        return r["expected"].get("qos_profile")

    for target, minimum in (("QI_STREAM_LIKE", 40), ("QI_FAILOVER", 30), ("QI_LOW_COST", 30)):
        have = sum(1 for r in buckets["sealed"] if qos_of(r) == target)
        guard = 0
        while have < minimum and guard < n:
            guard += 1
            cand = next((f for f in new_fams if f not in sealed_fams and f not in dev_fams
                         and f not in cal_fams
                         and any(qos_of(r) == target for r in by_fam[f])), None)
            if cand is None:
                break
            moved = by_fam[cand]
            buckets["sealed"].extend(moved)
            buckets["train"] = [r for r in buckets["train"]
                                if r["_meta"]["family"] != cand]
            sealed_fams.add(cand)
            have += sum(1 for r in moved if qos_of(r) == target)

    # ---- gravar train/dev/cal + manifest; sealed em local segregado ----
    manifest = {"date_seed": args.seed, "families_total_new_gen": n,
                "families": {"sealed": len(sealed_fams), "dev": len(dev_fams),
                             "calibration": len(cal_fams),
                             "train_new_gen": n - len(sealed_fams) - len(dev_fams) - len(cal_fams)},
                "splits": {}, "sealed_files": {}}
    for name in ("train", "dev", "calibration"):
        path = os.path.join(args.outdir, f"{name}.jsonl")
        with open(path, "w") as f:
            for r in sorted(buckets[name], key=lambda r: r["_meta"]["family"] + r["state"]["text"][:32]):
                f.write(json.dumps(r, ensure_ascii=False) + "\n")
        qd = Counter(qos_of(r) for r in buckets[name])
        manifest["splits"][name] = {"rows": len(buckets[name]), "qos_dist": dict(qd),
                                    "sha256": hashlib.sha256(open(path, "rb").read()).hexdigest()}

    sealed_path = os.path.join(args.sealed_out, "sealed_t904_pool.jsonl")
    labels_path = os.path.join(args.sealed_out, "sealed_t904_labels.json")
    with open(sealed_path, "w") as f:
        for r in sorted(buckets["sealed"], key=lambda r: r["_meta"]["family"] + r["state"]["text"][:32]):
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    labels = [{"family": r["_meta"]["family"], "expected": r["expected"],
               "lang": r["_meta"]["lang"], "stratum": r["_meta"]["stratum"]}
              for r in sorted(buckets["sealed"], key=lambda r: r["_meta"]["family"] + r["state"]["text"][:32])]
    with open(labels_path, "w") as f:
        json.dump(labels, f, ensure_ascii=False, indent=1)
    qd = Counter(qos_of(r) for r in buckets["sealed"])
    stream_literal = sum(1 for r in buckets["sealed"]
                         if "stream" in r["state"]["text"].lower())
    manifest["splits"]["sealed"] = {"rows": len(buckets["sealed"]), "qos_dist": dict(qd),
                                    "with_literal_stream": stream_literal,
                                    "sha256": hashlib.sha256(open(sealed_path, "rb").read()).hexdigest()}
    manifest["sealed_files"] = {
        "pool": {"path": sealed_path,
                 "sha256": hashlib.sha256(open(sealed_path, "rb").read()).hexdigest()},
        "labels": {"path": labels_path,
                   "sha256": hashlib.sha256(open(labels_path, "rb").read()).hexdigest()},
        "note": "abertura ÚNICA pré-registrada; treino/tuning não têm acesso a estes caminhos"}

    with open(os.path.join(args.outdir, "split_manifest.json"), "w") as f:
        json.dump(manifest, f, indent=1, ensure_ascii=False)
    os.chmod(sealed_path, 0o000)
    os.chmod(labels_path, 0o000)
    print(json.dumps(manifest, indent=1, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    import sys
    sys.exit(main())
