#!/usr/bin/env python3
"""T911 — GATES_FINAL: aplica os gates do PREREG_T911.json aos metrics.json dos 4 braços no sealed.

Uso: python gates_t911.py --eval-root /tmp/t911_evals --out GATES_FINAL_T911.json
Espera {ARM}_sealed/metrics.json com bootstrap_ci_per_class.
"""
import argparse, json, os

PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
CX = ["0", "1", "2", "3"]
URG = ["0", "1", "2"]
PLAN = ["True", "False"]
CAPS_AGG = ["True", "False"]
# não-inferioridade (PREREG): STREAM vs T910 B3 0.966; caps_agg não tem baseline direto
# — usa security_audit do T910 B4 (0.546) como referência do grupo caps, margem -0.05
NI_REF = {"qos:QI_STREAM_LIKE": 0.966}
T910_B1_REF = {"qos_macro": 0.773, "stream": 0.891, "urg_macro": 0.606, "plan_acc": 0.815,
               "cx_macro": 0.335, "caps": 0.529}


def class_f1(m, group, cls):
    per = m[group]["per_class"].get(str(cls)) or m[group]["per_class"].get(cls)
    return per["F1"] if per else 0.0


def all_classes(m):
    out = {}
    for c in PROFILES:
        out[f"qos:{c}"] = class_f1(m, "qos", c)
    for c in CX:
        out[f"cx:{c}"] = class_f1(m, "complexity", c)
    for c in URG:
        out[f"urg:{c}"] = class_f1(m, "urgency", c)
    for c in PLAN:
        per = m["planning"]["f1_per_class"].get(str(c)) or m["planning"]["f1_per_class"].get(c == "True")
        out[f"plan:{c}"] = per["F1"] if per else 0.0
    for c in CAPS_AGG:
        per = m["capabilities"]["per_class"].get(str(c)) or m["capabilities"]["per_class"].get(c == "True")
        out[f"caps_agg:{c}"] = per["F1"] if per else 0.0
    return out


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--eval-root", required=True)
    ap.add_argument("--arms", default="A0,A1,B0,B1")
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    arms = args.arms.split(",")
    table = {}
    for a in arms:
        p = os.path.join(args.eval_root, f"{a}_sealed", "metrics.json")
        if not os.path.exists(p):
            print(f"AVISO: {p} ausente")
            continue
        m = json.load(open(p))
        f1s = all_classes(m)
        ci = m.get("bootstrap_ci_per_class", {})
        min_f1 = min(f1s.values())
        gate_prim = all(f1s[k] > 0.90 and ci.get(k, {}).get("lo95", 0) > 0.88 for k in f1s)
        ni = {}
        for k, ref in NI_REF.items():
            lo = ci.get(k, {}).get("lo95", 0)
            ni[k] = {"ref": ref, "lo95": lo, "pass": lo >= ref - 0.05}
        table[a] = {
            "min_f1": round(min_f1, 4),
            "qos_macro": m["qos"]["macro_f1"], "cx_macro": m["complexity"]["macro_f1"],
            "urg_macro": m["urgency"]["macro_f1"], "plan_acc": m["planning"]["acc"],
            "caps_agg_macro": m["capabilities"]["macro_f1"],
            "per_class": f1s,
            "gate_primary_090_all_ci": gate_prim,
            "noninferiority": ni,
            "adapter_sha256": m.get("adapter_sha256", "?"),
        }

    cand = {a: v for a, v in table.items() if v["gate_primary_090_all_ci"]}
    if cand:
        best = max(cand, key=lambda a: cand[a]["min_f1"])
        decision = "PROMOTE"
    else:
        # comparação indireta com o teto do encoder (T910 B1) nos eixos ordinais:
        # o gargalo do encoder era urg/cx/plan; qos tem fome de dados documentada (6/6/4 no train)
        best = max(table, key=lambda a: table[a]["min_f1"])
        beats_ref = (table[best]["urg_macro"] > T910_B1_REF["urg_macro"] and
                     table[best]["cx_macro"] > T910_B1_REF["cx_macro"] and
                     table[best]["plan_acc"] > T910_B1_REF["plan_acc"])
        decision = "BLOCKED_WITH_TRAINED_CANDIDATE" if beats_ref else "BLOCKED"

    out = {
        "campaign": "T911",
        "gates_source": "PREREG_T911.json (commit 871efe8)",
        "sealed": "sealed/pool.jsonl 30ba7617a0e53045 + labels 1b4b04abfec47c65",
        "decision": decision,
        "finalista": best,
        "arms": table,
        "ref_T910_B1_indireto": T910_B1_REF,
        "note": "sealeds T910/T911 diferentes; comparação INDIRETA apenas",
    }
    json.dump(out, open(args.out, "w"), indent=1)
    hdr = f"{'arm':6} {'minF1':7} {'qos':6} {'cx':6} {'urg':6} {'plan':6} {'capsA':6} gate"
    print(hdr)
    for a, v in table.items():
        print(f"{a:6} {v['min_f1']:<7} {v['qos_macro']:<6} {v['cx_macro']:<6} {v['urg_macro']:<6} "
              f"{v['plan_acc']:<6} {v['caps_agg_macro']:<6} {v['gate_primary_090_all_ci']}")
    print(f"\nDECISION: {decision} | finalista: {best}")


if __name__ == "__main__":
    main()
