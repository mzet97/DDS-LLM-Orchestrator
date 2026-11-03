#!/usr/bin/env python3
"""T905-M2 — pool com rótulos MEDIDOS por construção + LLM-verificação.

qos: anotação LLM dupla + adjudicação (fronteira semântica, como T904).
urgency/complexity/needs_planning/caps: OURO DE CONSTRUÇÃO do cenário
(prazo do banco, nº de estágios, caps requeridas); a anotação LLM vira
VERIFICAÇÃO — concorda ⇒ tier measured-verified; diverge em algum qid medido
⇒ o qid medido prevalece (é gabarito do cenário) e a linha marca
`measured-llm-disagree` (auditoria).

Replay/reuso (train-only): sintéticos 2000 + t901 270 + hardnegs 70 + v4 1.200.
Auditoria 8-gram vs v4_test E vs sealed T904 (ambos locais).
"""
import argparse
import hashlib
import json
import re
import sys
from collections import Counter

sys.path.insert(0, "/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/src")
from laya_worker.questions import CAPABILITY_CATALOG, QUESTIONS_V1  # noqa: E402

NORM = re.compile(r"[^a-zà-ÿ0-9 ]+")
PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]


def norm_text(s):
    return NORM.sub(" ", s.lower()).strip()


def ngrams(s, n=8):
    w = s.split()
    return {" ".join(w[i:i + n]) for i in range(len(w) - n + 1)} if len(w) >= n else set()


def q_block():
    q = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
    for c in CAPABILITY_CATALOG:
        q[f"cap__{c}"] = {"type": "noul",
                          "instructions": f"Does this request require the capability '{c}'?"}
    return q


QUESTIONS = q_block()

PARTIAL_RX = re.compile(
    r"(parcial|parciais|incremental|incrementais|assim que (cada |termin|ficar|pronto)|conforme (ficar|produz|for)"
    r"|aos poucos|de peu[dx]s em peu[dx]s|partial results?|as soon as (each|they|it)|incrementally|"
    r"versões (parciais|intermediárias)|partial (outputs?|deliver|results?)|send (me )?(each|every)|"
    r"envie (cada|todas as? vers)|devolvendo as versões|each (transcri|analysis|stage|result))", re.I)
NOPARTIAL_RX = re.compile(
    r"(não (me )?(envie|mande|quero)[^.]*(parcial|antes)|sem (versões? )?(parciais|intermediár)|"
    r"nada parcial|resultado (final|completo) (no fim|ao final|de uma vez|único)|"
    r"do not send partial|no partial (outputs?|results?)|only (the )?final|single final|"
    r"at the end|ao final|no final|when (everything|done|complete)|quando (tudo|terminar)|"
    r"once everything|reply once|single deliverable)", re.I)
SUBJECT_STREAM_RX = re.compile(r"(kafka|kinesis|sse|stdout|syslog|streaming de vídeo|redis streams|"
                               r"stream processing|log stream|cdn cache)", re.I)


def adjudicate_qos(text, la, lb):
    if lb is None:
        return la["qos_profile"], "silver"
    if la["qos_profile"] == lb["qos_profile"]:
        return la["qos_profile"], "silver-dual"
    has_partial = bool(PARTIAL_RX.search(text))
    says_final = bool(NOPARTIAL_RX.search(text))
    if has_partial and not says_final:
        return "QI_STREAM_LIKE", "silver-adjudicated"
    if says_final and not has_partial:
        return "QI_BALANCED", "silver-adjudicated"
    return None, "review"


def row(text, expected, family, tier, partition_hint, lang, stratum, provenance, extra=None):
    return {"state": {"text": text}, "questions": QUESTIONS, "expected": expected,
            "_meta": {"family": family, "tier": tier, "partition_hint": partition_hint,
                      "lang": lang, "stratum": stratum, "provenance": provenance, **(extra or {})}}


def load(p):
    with open(p) as f:
        for line in f:
            if line.strip():
                yield json.loads(line)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--gen-dir", required=True)
    ap.add_argument("--t901", required=True)
    ap.add_argument("--synthetic", required=True)
    ap.add_argument("--v4train", required=True)
    ap.add_argument("--v4test", required=True)
    ap.add_argument("--t904-sealed", required=True, help="sealed T904 pool (PROIBIDO — auditoria)")
    ap.add_argument("--hardnegs", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--v4-max", type=int, default=1200)
    ap.add_argument("--seed", type=int, default=905)
    args = ap.parse_args()
    import os
    import random
    rng = random.Random(args.seed)
    rows, stats = [], Counter()

    # ---- T905 novos (medidos) ----
    la_map, lb_map = {}, {}
    for name, tgt in (("label_a.jsonl", la_map), ("label_b.jsonl", lb_map)):
        p = f"{args.gen_dir}/{name}"
        if os.path.exists(p):
            for r in load(p):
                tgt[r["id"].split(":s")[0]] = r.get("label", {})
    dual = []
    for r in load(f"{args.gen_dir}/gen.jsonl"):
        if "text" not in r:
            stats["t905_error"] += 1
            continue
        la = la_map.get(r["id"])
        if not la or "qos_profile" not in la:
            stats["t905_unlabeled"] += 1
            continue
        qos, status = adjudicate_qos(r["text"], la, lb_map.get(r["id"]))
        if qos is None:
            stats["t905_review_excluded"] += 1
            continue
        fam = "fam-" + hashlib.sha1(
            f"{r['domain']}|{r['delivery']}|{r['lang']}".encode()).hexdigest()[:10]
        # OUROS MEDIDOS prevalecem; LLM só confirma
        agree = {"urgency": la.get("urgency") == r["urgency_gold"],
                 "complexity": la.get("complexity") == r["complexity_gold"],
                 "needs_planning": bool(la.get("needs_planning")) == r["needs_planning_gold"],
                 "caps": all(bool(la.get("caps", {}).get(c, False)) == c_in
                             for c, c_in in ((c, r["caps_gold"].count(c) > 0)
                                             for c in CAPABILITY_CATALOG))}
        n_dis = sum(1 for v in agree.values() if not v)
        expected = {"qos_profile": qos,
                    "needs_planning": r["needs_planning_gold"],
                    "complexity": r["complexity_gold"],
                    "urgency": r["urgency_gold"],
                    **{f"cap__{c}": (c in r["caps_gold"]) for c in CAPABILITY_CATALOG}}
        rows.append(row(r["text"], expected, fam,
                        "measured" if n_dis == 0 else "measured-llm-disagree",
                        "new-gen", r["lang"], r["delivery"],
                        "t905-gen-measured-gemma4",
                        {"domain": r["domain"], "delivery_spec": r["delivery"],
                         "k_stages": r["k_stages"], "llm_agree": f"{4 - n_dis}/4"}))
        stats[f"t905_{status}"] += 1
        stats["t905_measured_agree_full"] += 1 if n_dis == 0 else 0
        stats["t905_measured_agree_partial"] += 1 if 0 < n_dis <= 2 else 0
        if status == "silver-dual":
            dual.append((la["qos_profile"], lb_map[r["id"]]["qos_profile"]))
    if dual:
        labs = sorted({q for p in dual for q in p})
        idx = {q: i for i, q in enumerate(labs)}
        n = len(labs)
        mat = [[0] * n for _ in range(n)]
        for a, b in dual:
            mat[idx[a]][idx[b]] += 1
        tot = len(dual)
        po = sum(mat[i][i] for i in range(n)) / tot
        pe = sum(sum(mat[i]) * sum(mat[j][i] for j in range(n)) for i in range(n)) / (tot * tot)
        stats["kappa_qos"] = round((po - pe) / (1 - pe) if pe < 1 else 1.0, 4)
        stats["dual_n"] = tot

    # ---- replay train-only (herdado da T904) ----
    for r in load(args.t901):
        exp = dict(r.get("expected") or {})
        if "qos_profile" not in exp:
            continue
        text = r["state"]["text"] if isinstance(r.get("state"), dict) else str(r.get("state"))
        rows.append(row(text, exp, "t901-" + hashlib.sha1(norm_text(text)[:120].encode()).hexdigest()[:10],
                        "reused-reviewed", "train-only", r.get("language", "en"),
                        "t901-contrastive", "t901-contrastive-reviewed"))
        stats["t901"] += 1
    for r in load(args.synthetic):
        exp = r.get("expected") or {}
        if "qos_profile" not in exp:
            continue
        text = r["state"]["text"] if isinstance(r.get("state"), dict) else str(r.get("state"))
        rows.append(row(text, exp, "syn-" + hashlib.sha1(norm_text(text)[:120].encode()).hexdigest()[:10],
                        "reused-synthetic", "train-only", "pt", "synthetic-replay",
                        "t900-calib-v3-synthetic"))
        stats["synthetic"] += 1
    v4 = []
    for r in load(args.v4train):
        exp = r.get("expected") or {}
        if exp.get("qos_profile") in ("QI_LOW_COST", "QI_BALANCED", "QI_CRITICAL"):
            text = r["state"]["text"] if isinstance(r.get("state"), dict) else str(r.get("state"))
            v4.append((exp["qos_profile"], text))
    by_q = {}
    for q, t in v4:
        by_q.setdefault(q, []).append(t)
    per = max(1, args.v4_max // max(1, len(by_q)))
    for q in sorted(by_q):
        rng.shuffle(by_q[q])
        for t in by_q[q][:per]:
            rows.append(row(t, {"qos_profile": q},
                            "v4-" + hashlib.sha1(norm_text(t)[:120].encode()).hexdigest()[:10],
                            "reused-real", "train-only", "en", "sprout-measured",
                            "sprout-v4-train-seed900"))
            stats[f"v4_{q}"] += 1
    for r in load(args.hardnegs):
        qos = "QI_STREAM_LIKE" if r["stratum"].startswith("pair-STREAM") else "QI_BALANCED"
        urg = 2 if ("ontem" in r["text"] or "2 hours" in r["text"]) else (
            0 if ("sem pressa" in r["text"] or "next month" in r["text"]) else 1)
        rows.append(row(r["text"], {"qos_profile": qos, "needs_planning": False,
                                    "complexity": 1, "urgency": urg,
                                    **{f"cap__{c}": False for c in CAPABILITY_CATALOG}},
                        r["family"], "template-hardneg", "train-only", r["lang"],
                        r["stratum"], "t904-deterministic-hardneg"))
        stats["hardneg"] += 1

    # ---- auditoria: 8-gram vs v4_test E vs sealed T904 (PROIBIDOS) ----
    stop = set()
    for p in (args.v4test, args.t904_sealed):
        if os.path.exists(p):
            for r in load(p):
                t = r["state"]["text"] if isinstance(r.get("state"), dict) else str(r.get("state"))
                stop |= ngrams(norm_text(t))
    seen_norm, leak_fams = {}, set()
    for i, r in enumerate(rows):
        t = norm_text(r["state"]["text"])
        if t in seen_norm:
            rows[i] = None
            stats["dedup_exact"] += 1
            continue
        seen_norm[t] = i
        if r["_meta"]["provenance"].startswith(("t905", "t901")) and (ngrams(t) & stop):
            leak_fams.add(r["_meta"]["family"])
    rows = [r for r in rows if r is not None]
    for r in rows:
        if r["_meta"]["family"] in leak_fams and r["_meta"]["provenance"].startswith(("t905", "t901")):
            r["_meta"]["tier"] = "review-leak-suspect"
            r["_meta"]["partition_hint"] = "excluded"
            stats["leak_excluded"] += 1
    stats["leak_families"] = len(leak_fams)

    with open(args.out, "w") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    qos_dist = Counter(r["expected"].get("qos_profile") for r in rows if "qos_profile" in r["expected"])
    summary = {"rows": len(rows), "qos_dist": dict(qos_dist), "stats": dict(stats),
               "families_new_gen": len({r["_meta"]["family"] for r in rows
                                        if r["_meta"]["partition_hint"] == "new-gen"})}
    json.dump(summary, open(args.out + ".summary.json", "w"), indent=1, ensure_ascii=False)
    print(json.dumps(summary, indent=1, ensure_ascii=False))
    return 0


if __name__ == "__main__":
    sys.exit(main())
