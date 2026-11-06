#!/usr/bin/env python3
"""T911 — Avaliador dos braços Unsloth Decision Model (Qwen3.5).

Espelha as definições do evaluator_t909.py (argmax choice/score, threshold 0.5 noul)
para comparabilidade T909/T910/T911. Roda na .61 (venv-t908) com FastDecisionModel.

Uso:
  python eval_t911.py --arm-dir /home/agent/t911_train/arm_A0 --prompt plain \
      --split /tmp/t909_data_t910/dev_t911.jsonl --out /tmp/t911_evals/A0_dev
  python eval_t911.py --arm-dir ... --prompt xml \
      --split /home/agent/t911_eval/sealed_pool.jsonl --labels /home/agent/t911_eval/sealed_labels.json \
      --out /tmp/t911_evals/A0_sealed --bootstrap 1000
"""
import argparse, hashlib, json, os, sys, time
from collections import defaultdict, Counter

sys.path.insert(0, "/tmp/t909_data_t910")

SEED = 909
PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
CAPS = ["rust_refactor", "python_refactor", "unit_testing", "security_audit",
        "code_review", "research", "summarization", "translation"]


def sha256_file(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for c in iter(lambda: f.read(1 << 20), b""):
            h.update(c)
    return h.hexdigest()


def load_split(path):
    return [json.loads(l) for l in open(path, errors="replace") if l.strip()]


def wrap_xml(t):
    import xml.etree.ElementTree as ET
    root = ET.Element("decision_input", version="t911-v1")
    ur = ET.SubElement(root, "user_request")
    ur.text = t
    return ET.tostring(root, encoding="unicode", short_empty_elements=True)


def parse_answers(ans):
    """Converte o output do FastDecisionModel.predict para o formato do evaluator_t909."""
    out = {}
    a = ans.get("qos_profile", {})
    out["qos"] = a.get("choice") or a.get("answer")
    probs = a.get("probabilities") or {}
    if not probs and isinstance(a.get("answer_distribution"), dict):
        probs = a["answer_distribution"]
    out["qos_probs"] = {str(k): round(float(v), 4) for k, v in probs.items()}
    out["qos_confidence"] = a.get("answer_confidence", a.get("confidence", 0))
    # noul: needs_planning (idêntico ao T909: p = ans["noul"]; plan = p >= 0.5)
    a = ans.get("needs_planning", {})
    p = float(a.get("noul", 0))
    out["plan"] = p >= 0.5
    out["plan_p"] = round(p, 4)
    # score: complexity
    a = ans.get("complexity", {})
    c_probs = {int(k): float(v) for k, v in (a.get("probabilities") or {}).items()}
    c_ev = float(a.get("score", 0))
    out["cx"] = int(max(c_probs, key=c_probs.get)) if c_probs else int(min(3, max(0, round(c_ev))))
    out["cx_probs"] = {str(k): round(c_probs[k], 4) for k in sorted(c_probs)}
    out["cx_ev"] = round(c_ev, 4)
    # score: urgency
    a = ans.get("urgency", {})
    u_probs = {int(k): float(v) for k, v in (a.get("probabilities") or {}).items()}
    u_ev = float(a.get("score", 0))
    out["urg"] = int(max(u_probs, key=u_probs.get)) if u_probs else int(min(2, max(0, round(u_ev))))
    out["urg_probs"] = {str(k): round(u_probs[k], 4) for k in sorted(u_probs)}
    out["urg_ev"] = round(u_ev, 4)
    # capabilities AGREGADA (formato QUESTIONS_V1 de 5 perguntas usado no treino T911)
    a = ans.get("capabilities", {})
    p = float(a.get("noul", 0))
    out["capabilities"] = p >= 0.5
    out["capabilities_p"] = round(p, 4)
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
    gamat = defaultdict(Counter)
    for row_id, p in preds.items():
        g = labels[row_id]
        qmat[g["qos_profile"]][p["qos"]] += 1
        cmat[int(g["complexity"])][int(p["cx"])] += 1
        umat[int(g["urgency"])][int(p["urg"])] += 1
        pmat[bool(g["needs_planning"])][bool(p["plan"])] += 1
        g_caps_agg = any(bool(g.get(f"cap__{c}")) for c in CAPS)
        gamat[g_caps_agg][bool(p["capabilities"])] += 1
    qos_macro, qos_per, qos_conf = _f1s(qmat, PROFILES)
    cx_macro, cx_per, cx_conf = _f1s(cmat, [0, 1, 2, 3])
    urg_macro, urg_per, urg_conf = _f1s(umat, [0, 1, 2])
    plan_macro, plan_per, _ = _f1s(pmat, [True, False])
    caps_macro, caps_per, _ = _f1s(gamat, [True, False])
    all_class_f1 = [v["F1"] for v in qos_per.values()] + [v["F1"] for v in cx_per.values()] \
        + [v["F1"] for v in urg_per.values()] + [v["F1"] for v in plan_per.values()] \
        + [v["F1"] for v in caps_per.values()]
    plan_acc = sum(1 for rid, p in preds.items() if bool(p["plan"]) == bool(labels[rid]["needs_planning"])) / len(preds)
    return {
        "n": len(preds),
        "min_f1_all_classes": round(min(all_class_f1), 4),
        "qos": {"macro_f1": round(qos_macro, 4), "per_class": qos_per, "confusion": qos_conf},
        "complexity": {"macro_f1": round(cx_macro, 4), "per_class": cx_per, "confusion": cx_conf},
        "urgency": {"macro_f1": round(urg_macro, 4), "per_class": urg_per, "confusion": urg_conf},
        "planning": {"acc": round(plan_acc, 4), "f1_per_class": plan_per},
        "capabilities": {"macro_f1": round(caps_macro, 4), "per_class": caps_per,
                         "note": "AGREGADA (any-cap); QUESTIONS_V1 de 5 perguntas, formato do treino/runtime"},
    }


def bootstrap_ci(preds, labels, n_boot=1000):
    """IC95 bootstrap por classe individual (percentil, seed fixa)."""
    import random
    rng = random.Random(SEED)
    ids = sorted(preds.keys())
    cache = [(rid, preds[rid], labels[rid]) for rid in ids]
    spec = []  # (name, classes, pred_key, gold_key_fn)
    for c in PROFILES:
        spec.append((f"qos:{c}", c, "qos", lambda g: g["qos_profile"]))
    for c in range(4):
        spec.append((f"cx:{c}", c, "cx", lambda g: int(g["complexity"])))
    for c in range(3):
        spec.append((f"urg:{c}", c, "urg", lambda g: int(g["urgency"])))
    for c in (True, False):
        spec.append((f"plan:{c}", c, "plan", lambda g: bool(g["needs_planning"])))
    spec.append(("caps_agg:True", True, "capabilities", lambda g: any(bool(g.get(f"cap__{c}")) for c in CAPS)))
    spec.append(("caps_agg:False", False, "capabilities", lambda g: any(bool(g.get(f"cap__{c}")) for c in CAPS)))

    def class_f1(mat, c):
        tp = mat[c].get(c, 0)
        fp = sum(mat[g].get(c, 0) for g in mat.keys() if g != c)
        fn = sum(mat[c].get(p, 0) for p in mat[c].keys() if p != c)
        prec = tp / (tp + fp) if tp + fp else 0.0
        rec = tp / (tp + fn) if tp + fn else 0.0
        return 2 * prec * rec / (prec + rec) if prec + rec else 0.0

    samples = {name: [] for name, *_ in spec}
    for b in range(n_boot):
        idx = [rng.randrange(len(cache)) for _ in range(len(cache))]
        for name, c, pred_key, gold_fn in spec:
            mat = defaultdict(Counter)
            for i in idx:
                rid, p, g = cache[i]
                mat[gold_fn(g)][p[pred_key]] += 1
            samples[name].append(class_f1(mat, c))
    out = {}
    for name, vals in samples.items():
        vals.sort()
        out[name] = {"lo95": round(vals[int(0.025 * len(vals))], 4),
                     "hi95": round(vals[int(0.975 * len(vals)) - 1], 4)}
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--arm-dir", required=True)
    ap.add_argument("--prompt", choices=["plain", "xml"], default="plain")
    ap.add_argument("--split", required=True)
    ap.add_argument("--labels", default=None, help="labels.json (sealed); se ausente, usa gold inline do split")
    ap.add_argument("--out", required=True)
    ap.add_argument("--limit", type=int, default=0)
    ap.add_argument("--bootstrap", type=int, default=0)
    args = ap.parse_args()

    os.makedirs(args.out, exist_ok=True)
    from unsloth import FastDecisionModel
    from laya_worker.questions import QUESTIONS_V1

    rows = load_split(args.split)
    if args.limit:
        rows = rows[: args.limit]
    if args.labels:
        labels = {k: v for k, v in json.load(open(args.labels)).items()}
        assert len(labels) == len(rows), f"labels {len(labels)} != rows {len(rows)}"
    else:
        labels = {f"row:{i}": r["gold"] for i, r in enumerate(rows)}

    print(f"[eval] loading {args.arm_dir} (prompt={args.prompt})", flush=True)
    model, tokenizer = FastDecisionModel.from_pretrained(model_name=args.arm_dir, max_seq_length=2048)
    FastDecisionModel.for_inference(model)

    preds, raw0 = {}, None
    t0 = time.time()
    for i, r in enumerate(rows):
        rid = f"row:{i}"
        state = r["state"]["text"] if isinstance(r.get("state"), dict) else r.get("state", "")
        if args.prompt == "xml":
            state = wrap_xml(state)
        Q = r.get("questions") or QUESTIONS_V1
        ans = FastDecisionModel.predict(model, tokenizer, state, Q)
        if raw0 is None:
            raw0 = ans
            with open(os.path.join(args.out, "raw_first_prediction.json"), "w") as f:
                json.dump(ans, f, indent=1, default=str)
        preds[rid] = parse_answers(ans)
        if (i + 1) % 200 == 0:
            el = time.time() - t0
            print(f"[eval] {i+1}/{len(rows)} {el:.0f}s ({el/(i+1):.2f}s/ex)", flush=True)

    with open(os.path.join(args.out, "predictions.jsonl"), "w") as f:
        for rid, p in preds.items():
            f.write(json.dumps({"row_id": rid, "pred": p}, ensure_ascii=False) + "\n")

    m = metrics(preds, labels)
    m["arm"] = os.path.basename(args.arm_dir.rstrip("/"))
    m["prompt"] = args.prompt
    m["split"] = args.split
    m["split_sha256"] = sha256_file(args.split)
    m["adapter_sha256"] = sha256_file(os.path.join(args.arm_dir, "adapter_model.safetensors"))
    if args.bootstrap:
        m["bootstrap_ci_per_class"] = bootstrap_ci(preds, labels, args.bootstrap)
    with open(os.path.join(args.out, "metrics.json"), "w") as f:
        json.dump(m, f, indent=1)
    print(f"[eval] DONE min_f1={m['min_f1_all_classes']} qos={m['qos']['macro_f1']} "
          f"cx={m['complexity']['macro_f1']} urg={m['urgency']['macro_f1']} "
          f"plan={m['planning']['acc']} caps_agg={m['capabilities']['macro_f1']}", flush=True)


if __name__ == "__main__":
    main()
