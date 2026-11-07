#!/usr/bin/env python3
"""T912-F4/F5 — Avaliador 12 perguntas (4 eixos + 8 caps individuais).

Espelha evaluator_t909 (argmax choice/score, threshold 0.5 noul) + per-cap F1.
Roda na RX (venv-t909-rocm) ou na .61 (venv-t908).

Uso:
  eval_t912.py --arm-dir CKPT --split datasets/dev_t912.jsonl --out OUT [--limit N]
  eval_t912.py --arm-dir CKPT --split sealed/pool.jsonl --labels sealed/labels.json \
      --out OUT --bootstrap 1000
"""
import argparse
import hashlib
import json
import os
import sys
import time
from collections import defaultdict, Counter

sys.path.insert(0, "/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/src")
from laya_worker.questions import QUESTIONS_V1, CAPABILITY_CATALOG

SEED = 909
PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
CAPS = list(CAPABILITY_CATALOG)
Q12 = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
for c in CAPS:
    Q12[f"cap__{c}"] = {"type": "noul",
                        "instructions": f"Does this request require the capability '{c}'?"}


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for c in iter(lambda: f.read(1 << 20), b""):
            h.update(c)
    return h.hexdigest()


def load_split(path):
    return [json.loads(l) for l in open(path, errors="replace") if l.strip()]


def parse_answers(ans):
    out = {}
    a = ans.get("qos_profile", {})
    out["qos"] = a.get("choice") or a.get("answer")
    out["qos_probs"] = {str(k): round(float(v), 4) for k, v in (a.get("probabilities") or {}).items()}
    a = ans.get("needs_planning", {})
    p = float(a.get("noul", 0))
    out["plan"] = p >= 0.5
    out["plan_p"] = round(p, 4)
    a = ans.get("complexity", {})
    c_probs = {int(k): float(v) for k, v in (a.get("probabilities") or {}).items()}
    out["cx"] = int(max(c_probs, key=c_probs.get)) if c_probs else int(min(3, max(0, round(float(a.get("score", 0))))))
    out["cx_probs"] = {str(k): round(c_probs[k], 4) for k in sorted(c_probs)}
    a = ans.get("urgency", {})
    u_probs = {int(k): float(v) for k, v in (a.get("probabilities") or {}).items()}
    out["urg"] = int(max(u_probs, key=u_probs.get)) if u_probs else int(min(2, max(0, round(float(a.get("score", 0))))))
    out["urg_probs"] = {str(k): round(u_probs[k], 4) for k in sorted(u_probs)}
    for c in CAPS:
        a = ans.get(f"cap__{c}", {})
        p = float(a.get("noul", 0))
        out[f"cap__{c}"] = p >= 0.5
        out[f"cap__{c}_p"] = round(p, 4)
    return out


def _f1s(mat, classes):
    f1s, per = [], {}
    for c in classes:
        tp = mat[c].get(c, 0)
        fp = sum(mat[g].get(c, 0) for g in classes if g != c)
        fn = sum(mat[c].get(p, 0) for p in classes if p != c)
        prec = tp / (tp + fp) if tp + fp else 0.0
        rec = tp / (tp + fn) if tp + fn else 0.0
        f1 = 2 * prec * rec / (prec + rec) if prec + rec else 0.0
        f1s.append(f1)
        per[str(c)] = {"P": round(prec, 4), "R": round(rec, 4), "F1": round(f1, 4),
                       "support": tp + fn}
    return sum(f1s) / len(f1s), per, {str(c): dict(mat[c]) for c in classes}


def metrics(preds, labels):
    qmat, cmat, umat, pmat = defaultdict(Counter), defaultdict(Counter), defaultdict(Counter), defaultdict(Counter)
    capmats = {c: defaultdict(Counter) for c in CAPS}
    for rid, p in preds.items():
        g = labels[rid]
        qmat[g["qos_profile"]][p["qos"]] += 1
        cmat[int(g["complexity"])][int(p["cx"])] += 1
        umat[int(g["urgency"])][int(p["urg"])] += 1
        pmat[bool(g["needs_planning"])][bool(p["plan"])] += 1
        for c in CAPS:
            k = f"cap__{c}"
            capmats[c][bool(g.get(k))][bool(p[k])] += 1
    qos_macro, qos_per, qos_conf = _f1s(qmat, PROFILES)
    cx_macro, cx_per, cx_conf = _f1s(cmat, [0, 1, 2, 3])
    urg_macro, urg_per, urg_conf = _f1s(umat, [0, 1, 2])
    plan_macro, plan_per, _ = _f1s(pmat, [True, False])
    caps_f1 = {}
    for c in CAPS:
        _, per, _ = _f1s(capmats[c], [True, False])
        caps_f1[c] = per  # per["True"]/["False"]
    caps_pos_macro = sum(v["True"]["F1"] for v in caps_f1.values()) / len(CAPS)
    all_f1 = [v["F1"] for v in qos_per.values()] + [v["F1"] for v in cx_per.values()] \
        + [v["F1"] for v in urg_per.values()] + [v["F1"] for v in plan_per.values()] \
        + [v["True"]["F1"] for v in caps_f1.values()]
    plan_acc = sum(1 for rid, p in preds.items()
                   if bool(p["plan"]) == bool(labels[rid]["needs_planning"])) / len(preds)
    return {
        "n": len(preds), "min_f1_all_classes": round(min(all_f1), 4),
        "qos": {"macro_f1": round(qos_macro, 4), "per_class": qos_per, "confusion": qos_conf},
        "complexity": {"macro_f1": round(cx_macro, 4), "per_class": cx_per, "confusion": cx_conf},
        "urgency": {"macro_f1": round(urg_macro, 4), "per_class": urg_per, "confusion": urg_conf},
        "planning": {"acc": round(plan_acc, 4), "f1_per_class": plan_per},
        "caps": {"pos_macro_f1": round(caps_pos_macro, 4), "per_cap": caps_f1},
    }


def bootstrap_ci(preds, labels, n_boot=1000):
    import random
    rng = random.Random(SEED)
    ids = sorted(preds.keys())
    cache = [(rid, preds[rid], labels[rid]) for rid in ids]
    spec = []
    for c in PROFILES:
        spec.append((f"qos:{c}", c, "qos", lambda g: g["qos_profile"], list(PROFILES)))
    for c in range(4):
        spec.append((f"cx:{c}", c, "cx", lambda g: int(g["complexity"]), list(range(4))))
    for c in range(3):
        spec.append((f"urg:{c}", c, "urg", lambda g: int(g["urgency"]), list(range(3))))
    for c in (True, False):
        spec.append((f"plan:{c}", c, "plan", lambda g: bool(g["needs_planning"]), [True, False]))
    for cap in CAPS:
        k = f"cap__{cap}"
        spec.append((f"cap:{cap}:True", True, k, (lambda kk: (lambda g: bool(g.get(kk))))(k), [True, False]))

    def class_f1(mat, classes, c):
        tp = mat[c].get(c, 0)
        fp = sum(mat[g].get(c, 0) for g in classes if g != c)
        fn = sum(mat[c].values()) - tp
        prec = tp / (tp + fp) if tp + fp else 0.0
        rec = tp / (tp + fn) if tp + fn else 0.0
        return 2 * prec * rec / (prec + rec) if prec + rec else 0.0

    samples = {name: [] for name, *_ in spec}
    for b in range(n_boot):
        idx = [rng.randrange(len(cache)) for _ in range(len(cache))]
        for name, c, pk, gfn, classes in spec:
            mat = defaultdict(Counter)
            for i in idx:
                rid, p, g = cache[i]
                mat[gfn(g)][p[pk]] += 1
            samples[name].append(class_f1(mat, classes, c))
    out = {}
    for name, vals in samples.items():
        vals.sort()
        out[name] = {"lo95": round(vals[int(0.025 * len(vals))], 4),
                     "hi95": round(vals[int(0.975 * len(vals)) - 1], 4)}
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm-dir", required=True)
    ap.add_argument("--split", required=True)
    ap.add_argument("--labels", default=None)
    ap.add_argument("--out", required=True)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--bootstrap", type=int, default=0)
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)

    from unsloth import FastDecisionModel
    rows = load_split(args.split)
    if args.limit:
        rows = rows[: args.limit]
    if args.labels:
        labels = json.load(open(args.labels))
    else:
        labels = {f"row:{i}": r["gold"] for i, r in enumerate(rows)}

    print(f"[eval] loading {args.arm_dir}", flush=True)
    model, tokenizer = FastDecisionModel.from_pretrained(model_name=args.arm_dir, max_seq_length=2048)
    FastDecisionModel.for_inference(model)

    preds = {}
    t0 = time.time()
    for i, r in enumerate(rows):
        rid = r.get("_row_id", f"row:{i}")
        state = r["state"]["text"] if isinstance(r.get("state"), dict) else r.get("state", "")
        ans = FastDecisionModel.predict(model, tokenizer, state, Q12)
        preds[rid] = parse_answers(ans)
        if (i + 1) % 200 == 0:
            el = time.time() - t0
            print(f"[eval] {i+1}/{len(rows)} {el:.0f}s ({el/(i+1):.2f}s/ex)", flush=True)

    with open(os.path.join(args.out, "predictions.jsonl"), "w") as f:
        for rid, p in preds.items():
            f.write(json.dumps({"row_id": rid, "pred": p}, ensure_ascii=False) + "\n")

    m = metrics(preds, labels)
    m["arm"] = os.path.basename(args.arm_dir.rstrip("/"))
    m["split"] = args.split
    m["split_sha256"] = sha256_file(args.split)
    m["adapter_sha256"] = sha256_file(os.path.join(args.arm_dir, "adapter_model.safetensors"))
    if args.bootstrap:
        m["bootstrap_ci_per_class"] = bootstrap_ci(preds, labels, args.bootstrap)
    with open(os.path.join(args.out, "metrics.json"), "w") as f:
        json.dump(m, f, indent=1)
    caps_line = " ".join(f"{c}={m['caps']['per_cap'][c]['True']['F1']:.2f}" for c in CAPS)
    print(f"[eval] DONE minF1={m['min_f1_all_classes']} qos={m['qos']['macro_f1']} "
          f"cx={m['complexity']['macro_f1']} urg={m['urgency']['macro_f1']} "
          f"plan={m['planning']['acc']} caps[{caps_line}]", flush=True)


if __name__ == "__main__":
    main()
