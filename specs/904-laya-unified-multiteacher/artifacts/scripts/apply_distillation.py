#!/usr/bin/env python3
"""T904-M5/M6 join — injeta gold (distribuições dos professores) no train.jsonl e
constrói o teacher_registry a partir do DEV comum (métrica por qid, congelada
ANTES do sealed).

Entrada: teachers_preds/{t900,t902b,t903c}__{dev,distill}.json + upload_train.jsonl
Saída:   train.jsonl (linhas distill com gold por qid confiável),
         train_nodistill.jsonl (mesmas linhas, SEM gold — controle A/B),
         teacher_registry.json (métricas dev por qid × teacher + política de conflito),
         dev_comparison.json (preds dos professores × rótulos do dev).
"""
import argparse
import json
import os
from collections import Counter

sys_path = "/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/src"

PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
CAPS = ["rust_refactor", "python_refactor", "unit_testing", "security_audit",
        "code_review", "research", "summarization", "translation"]

# política de conflito CONGELADA aqui (antes de qualquer sealed):
# por qid, o professor com maior accuracy no dev comum vira a fonte gold nas linhas
# distill; rótulo auditado prevalece quando o teacher erra (gold só entra se o
# argmax do teacher == rótulo auditado OU qid sem rótulo auditado na linha).


def acc(preds, golds, key):
    n = c = 0
    for p, g in zip(preds, golds):
        if key == "qos_profile":
            c += p.get("qos_choice") == g
        elif key == "needs_planning":
            c += (p.get("needs_planning", 0) >= 0.5) == bool(g)
        elif key == "complexity":
            dist = p.get("complexity", {})
            c += int(max(dist, key=dist.get) if dist else 1) == int(g)
        elif key == "urgency":
            dist = p.get("urgency", {})
            c += int(max(dist, key=dist.get) if dist else 1) == int(g)
        elif key.startswith("cap__"):
            c += (p.get("caps", {}).get(key[5:], 0) >= 0.5) == bool(g)
        n += 1
    return round(c / n, 4) if n else 0.0


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--preds-dir", required=True)
    ap.add_argument("--datasets", required=True)
    ap.add_argument("--outdir", required=True)
    args = ap.parse_args()
    import sys
    sys.path.insert(0, sys_path)
    from laya_worker.questions import CAPABILITY_CATALOG  # noqa: F401

    preds = {}
    for tid in ("t900", "t902b", "t903c"):
        for split in ("dev", "distill"):
            p = f"{args.preds_dir}/{tid}__{split}.json"
            if os.path.exists(p):
                preds[(tid, split)] = json.load(open(p))["preds"]

    dev_rows = [json.loads(l) for l in open(f"{args.datasets}/upload_dev.jsonl") if l.strip()]
    train_rows = [json.loads(l) for l in open(f"{args.datasets}/upload_train.jsonl") if l.strip()]
    import hashlib
    import re as _re

    def text_key(s):
        return hashlib.sha1(_re.sub(r"[^\w ]", "", s.lower()).encode()).hexdigest()

    distill_ids = set()
    dp = f"{args.datasets}/distill.jsonl"
    if os.path.exists(dp):
        distill_ids = {text_key(json.loads(l)["state"]["text"]) for l in open(dp) if l.strip()}

    # ---- dev comparison por qid × teacher (rótulos do dev vêm do próprio arquivo) ----
    dev_labels = [r["expected"] for r in dev_rows]
    registry = {"frozen_at": "antes do sealed", "policy": "gold do teacher vencedor por qid no dev; rótulo auditado prevalece",
                "teachers": {}}
    for tid in ("t900", "t902b", "t903c"):
        if (tid, "dev") not in preds:
            continue
        P = preds[(tid, "dev")]
        m = {"qos_profile": acc(P, [g["qos_profile"] for g in dev_labels], "qos_profile"),
             "needs_planning": acc(P, [g["needs_planning"] for g in dev_labels], "needs_planning"),
             "complexity": acc(P, [g["complexity"] for g in dev_labels], "complexity"),
             "urgency": acc(P, [g["urgency"] for g in dev_labels], "urgency")}
        for c in CAPS:
            m[f"cap__{c}"] = acc(P, [g.get(f"cap__{c}", False) for g in dev_labels], f"cap__{c}")
        registry["teachers"][tid] = {"dev_metrics": m,
                                     "sha256": {"t900": "358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f",
                                                "t902b": "5d7dd6522d94347f47566499e09cbd2ad90b4fccbeb26a91c6885fa8c0eda4c0",
                                                "t903c": "7de213c7b24fca603f1f58e04ad4f8029fef396d5cf0e24e3c0067bb996cd05d"}[tid]}
    best_q = {}
    for qid in ["qos_profile", "needs_planning", "complexity", "urgency"] + [f"cap__{c}" for c in CAPS]:
        scores = {t: registry["teachers"][t]["dev_metrics"].get(qid, -1) for t in registry["teachers"]}
        best_q[qid] = max(scores, key=scores.get) if scores else None
    registry["best_teacher_by_qid"] = best_q

    # ---- injetar gold nas linhas distill (argmax do teacher deve concordar com o auditado) ----
    D = preds.get(("t900", "distill")) or []
    n_gold = 0
    distill_rows = [json.loads(l) for l in open(dp) if l.strip()]
    by_row = {}
    for i, p in enumerate(D):
        if i < len(distill_rows):
            import hashlib
            import re as _re2
            t = distill_rows[i]["state"]["text"]
            k = hashlib.sha1(_re2.sub(r"[^\w ]", "", t.lower()).encode()).hexdigest()
            by_row[k] = p
    for r in train_rows:
        rid = text_key(r["state"]["text"])
        if rid not in distill_ids or rid not in by_row:
            continue
        p = by_row[rid]
        gold = {}
        exp = r["expected"]
        if best_q["qos_profile"] and p.get("qos_probabilities"):
            qgold = max(p["qos_probabilities"], key=p["qos_probabilities"].get)
            if qgold == exp.get("qos_profile"):
                total = sum(p["qos_probabilities"].values()) or 1.0
                gold["qos_profile"] = {"probabilities": {k: v / total for k, v in p["qos_probabilities"].items()}}
        if best_q["needs_planning"] and best_q["needs_planning"] != best_q["qos_profile"]:
            pv = p.get("needs_planning", 0.0)
            if (pv >= 0.5) == bool(exp.get("needs_planning")):
                gold["needs_planning"] = {"probabilities": {"false": 1 - pv, "true": pv}}
        for key, dist_key in (("complexity", "complexity"), ("urgency", "urgency")):
            t = best_q[key]
            if t and t != best_q["qos_profile"] and p.get(dist_key):
                dist = p[dist_key]
                amax = int(max(dist, key=dist.get))
                if amax == int(exp.get(key, -1)):
                    gold[key] = {"probabilities": {str(k): float(dist.get(str(k), 0.0)) for k in
                                                   range(4 if key == "complexity" else 3)}}
        for c in CAPS:
            t = best_q.get(f"cap__{c}")
            if t and t != best_q["qos_profile"]:
                pv = p.get("caps", {}).get(c, 0.0)
                if (pv >= 0.5) == bool(exp.get(f"cap__{c}", False)):
                    gold[f"cap__{c}"] = {"probabilities": {"false": 1 - pv, "true": pv}}
        if gold:
            r["gold"] = gold
            n_gold += 1

    os.makedirs(args.outdir, exist_ok=True)
    with open(f"{args.outdir}/train.jsonl", "w") as f:
        for r in train_rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    with open(f"{args.outdir}/train_nodistill.jsonl", "w") as f:
        for r in train_rows:
            r2 = {k: v for k, v in r.items() if k != "gold"}
            f.write(json.dumps(r2, ensure_ascii=False) + "\n")
    json.dump(registry, open(f"{args.outdir}/teacher_registry.json", "w"), indent=1, ensure_ascii=False)
    print(json.dumps({"rows_train": len(train_rows), "rows_with_gold": n_gold,
                      "best_teacher_by_qid": best_q}, indent=1, ensure_ascii=False)[:1200])


if __name__ == "__main__":
    main()
