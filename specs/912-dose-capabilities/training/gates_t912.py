#!/usr/bin/env python3
"""T912-F5 — GATES_FINAL: aplica PREREG_T912 aos metrics.json dos braços no sealed.

Gates: F1 > 0.90 nas 22 classes (5 qos + 4 cx + 3 urg + 2 plan + 8 caps positiva)
com IC95 bootstrap lower > 0.88. Não-inferioridade indireta vs T911 A0 sealed
(margem -0.05) em urg/cx/plan macro (flagged indireto).
"""
import argparse
import json
import os

PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
CAPS = ["rust_refactor", "python_refactor", "unit_testing", "security_audit",
        "code_review", "research", "summarization", "translation"]
T911_A0_REF = {"urg_macro": 0.850, "cx_macro": 0.605, "plan_acc": 0.913}
NI_MARGIN = 0.05


def all_classes(m):
    out = {}
    for c in PROFILES:
        out[f"qos:{c}"] = m["qos"]["per_class"][c]["F1"]
    for c in ("0", "1", "2", "3"):
        out[f"cx:{c}"] = m["complexity"]["per_class"][c]["F1"]
    for c in ("0", "1", "2"):
        out[f"urg:{c}"] = m["urgency"]["per_class"][c]["F1"]
    for c in ("True", "False"):
        out[f"plan:{c}"] = m["planning"]["f1_per_class"][c]["F1"]
    for c in CAPS:
        out[f"cap:{c}:True"] = m["caps"]["per_cap"][c]["True"]["F1"]
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--eval-root", required=True)
    ap.add_argument("--arms", default="C0,C1")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    table = {}
    for a in args.arms.split(","):
        p = os.path.join(args.eval_root, f"{a}_sealed", "metrics.json")
        if not os.path.exists(p):
            print(f"AVISO: {p} ausente")
            continue
        m = json.load(open(p))
        f1s = all_classes(m)
        ci = m.get("bootstrap_ci_per_class", {})
        passing = {k: (v, ci.get(k, {}).get("lo95", 0)) for k, v in f1s.items()}
        gate_prim = all(v > 0.90 and lo > 0.88 for v, lo in passing.values())
        ni = {}
        for name, key, ref in (("urg_macro", "urgency", T911_A0_REF["urg_macro"]),
                               ("cx_macro", "complexity", T911_A0_REF["cx_macro"]),
                               ("plan_acc", "planning", T911_A0_REF["plan_acc"])):
            obs = m[key]["macro_f1"] if name != "plan_acc" else m[key]["acc"]
            ni[name] = {"ref_t911_a0": ref, "obs": obs,
                        "pass": obs >= ref - NI_MARGIN}
        table[a] = {
            "min_f1": min(f1s.values()),
            "qos_macro": m["qos"]["macro_f1"], "cx_macro": m["complexity"]["macro_f1"],
            "urg_macro": m["urgency"]["macro_f1"], "plan_acc": m["planning"]["acc"],
            "caps_pos_macro": m["caps"]["pos_macro_f1"],
            "per_class": f1s,
            "classes_passing_090": sum(1 for v in f1s.values() if v > 0.90),
            "classes_total": len(f1s),
            "gate_primary_090_all_ci": gate_prim,
            "noninferiority_indirect_t911": ni,
            "adapter_sha256": m.get("adapter_sha256", "?"),
        }

    cand = {a: v for a, v in table.items() if v["gate_primary_090_all_ci"]}
    if cand:
        best = max(cand, key=lambda a: cand[a]["min_f1"])
        decision = "PROMOTE_CANDIDATE"  # promocao final exige F6 compat+E2E verdes
    else:
        best = max(table, key=lambda a: table[a]["min_f1"])
        ni_ok = all(v["pass"] for v in table[best]["noninferiority_indirect_t911"].values())
        decision = "BLOCKED_WITH_TRAINED_CANDIDATE" if ni_ok else "BLOCKED"

    out = {
        "campaign": "T912",
        "gates_source": "PREREG_T912.json (commit antes do treino)",
        "decision": decision,
        "finalista": best,
        "arms": table,
        "ref_T911_A0_indireto": T911_A0_REF,
        "note": "comparacao T911 e INDIRETA (sealeds diferentes); PROMOTE final requer F6 compat worker+shadow+E2E",
    }
    json.dump(out, open(args.out, "w"), indent=1)
    print(f"{'arm':5} {'minF1':7} {'qos':6} {'cx':6} {'urg':6} {'plan':6} {'capsM':6} pass090 gate")
    for a, v in table.items():
        print(f"{a:5} {v['min_f1']:<7} {v['qos_macro']:<6} {v['cx_macro']:<6} {v['urg_macro']:<6} "
              f"{v['plan_acc']:<6} {v['caps_pos_macro']:<6} "
              f"{v['classes_passing_090']}/{v['classes_total']:>2} {v['gate_primary_090_all_ci']}")
    print(f"\nDECISION: {decision} | finalista: {best}")


if __name__ == "__main__":
    main()
