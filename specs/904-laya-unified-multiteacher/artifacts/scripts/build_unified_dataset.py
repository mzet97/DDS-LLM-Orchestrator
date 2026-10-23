#!/usr/bin/env python3
"""T904-M2 — constrói o pool unificado com proveniência e auditoria anti-vazamento.

Entradas:
  --gen-dir /tmp/laya_t904_gen (gen.jsonl, label_a.jsonl, label_b.jsonl — da .61)
  --t901 <t901/contrastive.jsonl>          (reused-reviewed, train-only)
  --synthetic <calib_v3/dataset_calib_v2.jsonl>  (reused-synthetic, train-only)
  --v4train <calib_v4/v4_train.jsonl>      (real SPROUT, rótulo SÓ qos, train-only)
  --v4test  <calib_v4/v4_test.jsonl>       (PROIBIDO — usado SÓ como referência de leak audit)
  --hardnegs <hardnegs.jsonl>              (template-hardneg, train-only)
Saída: unified_pool.jsonl — linhas SDK {state, questions, expected} + metadados
{_meta: {family, tier, partition_hint, lang, stratum, provenance}} + relatório.
Rótulos parciais por linha são suportados pelo SDK (qids ausentes não treinam).
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


def norm_text(s: str) -> str:
    return NORM.sub(" ", s.lower()).strip()


def ngrams(s: str, n: int = 8):
    w = s.split()
    return {" ".join(w[i:i + n]) for i in range(len(w) - n + 1)} if len(w) >= n else set()


def q_block() -> dict:
    """QUESTIONS_V1 no formato do SDK (choice/noul/score + cap__*), byte-exato."""
    q = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
    for cap in CAPABILITY_CATALOG:
        q[f"cap__{cap}"] = {"type": "noul",
                            "instructions": f"Does this request require the capability '{cap}'?"}
    return q


QUESTIONS = q_block()


def row(text, expected, family, tier, partition_hint, lang, stratum, provenance, extra=None):
    return {"state": {"text": text}, "questions": QUESTIONS, "expected": expected,
            "_meta": {"family": family, "tier": tier, "partition_hint": partition_hint,
                      "lang": lang, "stratum": stratum, "provenance": provenance,
                      **(extra or {})}}


# ---- adjudicação determinística de divergências (só regra de texto + política) ----
PARTIAL_RX = re.compile(
    r"(parcial|parciais|incremental|incrementais|assim que (cada |termin|ficar|pronto)|conforme (ficar|produz|for)"
    r"|aos poucos|de peu[dx]s em peu[dx]s|partial results?|as soon as (each|they|it)|incrementally|"
    r"versões (parciais|intermediárias)|partial (outputs?|deliver|results?)|send (me )?(each|every)"
    r"|envie (cada|todas as? vers)|devolvendo as versões)", re.I)
NOPARTIAL_RX = re.compile(
    r"(não (me )?(envie|mande|quero)[^.]*(parcial|antes)|sem (versões? )?(parciais|intermediár)|"
    r"nada parcial|resultado (final|completo) (no fim|ao final|de uma vez|único)|"
    r"do not send partial|no partial (outputs?|results?)|only (the )?final|single final|"
    r"at the end|ao final|no final|when (everything|done|complete)|quando (tudo|terminar))", re.I)
SUBJECT_STREAM_RX = re.compile(r"(kafka|kinesis|sse|stdout|syslog|streaming de vídeo|redis streams|"
                               r"stream processing|log stream)", re.I)


def adjudicate(text: str, la: dict, lb: dict | None) -> tuple[str | None, str]:
    """Retorna (qos_final, status). Regras da labeling_policy_v3; None+review = excluir."""
    if lb is None:
        return la["qos_profile"], "silver"
    if la["qos_profile"] == lb["qos_profile"]:
        return la["qos_profile"], "silver-dual"
    has_partial = bool(PARTIAL_RX.search(text))
    says_final_only = bool(NOPARTIAL_RX.search(text))
    if has_partial and not says_final_only:
        return "QI_STREAM_LIKE", "silver-adjudicated"
    if says_final_only and not has_partial:
        return "QI_BALANCED", "silver-adjudicated"
    return None, "review"


def load_jsonl(p):
    with open(p) as f:
        for line in f:
            line = line.strip()
            if line:
                yield json.loads(line)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--gen-dir", required=True)
    ap.add_argument("--t901", required=True)
    ap.add_argument("--synthetic", required=True)
    ap.add_argument("--v4train", required=True)
    ap.add_argument("--v4test", required=True)
    ap.add_argument("--hardnegs", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--v4-max", type=int, default=1500)
    ap.add_argument("--seed", type=int, default=904)
    args = ap.parse_args()

    import random
    rng = random.Random(args.seed)
    rows, stats = [], Counter()

    # ---- C1: novos contrastivos (gen + labels; + suplemento direcionado) ----
    import os

    def merge_gen(gen_path, la_path, lb_path, fam_key, provenance, stat_prefix, delivery_field):
        la_map, lb_map = {}, {}
        for name, tgt in ((la_path, la_map), (lb_path, lb_map)):
            if not os.path.exists(name):
                continue
            for r in load_jsonl(name):
                tgt[r["id"].split(":s")[0]] = r.get("label", {})
        dual = []
        for r in load_jsonl(gen_path):
            if "text" not in r:
                stats[f"{stat_prefix}_error"] += 1
                continue
            la = la_map.get(r["id"])
            if not la or "qos_profile" not in la:
                stats[f"{stat_prefix}_unlabeled"] += 1
                continue
            qos, status = adjudicate(r["text"], la, lb_map.get(r["id"]))
            if qos is None:
                stats[f"{stat_prefix}_review_excluded"] += 1
                continue
            fam = "fam-" + hashlib.sha1(fam_key(r).encode()).hexdigest()[:10]
            expected = {"qos_profile": qos, "needs_planning": la["needs_planning"],
                        "complexity": la["complexity"], "urgency": la["urgency"],
                        **{f"cap__{c}": la["caps"][c] for c in CAPABILITY_CATALOG}}
            rows.append(row(r["text"], expected, fam, status,
                            "new-gen", r["lang"],
                            r.get("stratum") or r.get("qos_family") or r.get(delivery_field, ""),
                            provenance,
                            {"domain": r["domain"], "delivery_spec": r.get(delivery_field, r.get("qos_family", "")),
                             "conf": la.get("confidence", 0.5)}))
            stats[f"{stat_prefix}_{status}"] += 1
            if status == "silver-dual":
                dual.append((la["qos_profile"], lb_map[r["id"]]["qos_profile"]))
        return dual

    dual_qos = merge_gen(f"{args.gen_dir}/gen.jsonl",
                         f"{args.gen_dir}/label_a.jsonl", f"{args.gen_dir}/label_b.jsonl",
                         lambda r: f"{r['domain']}|{r['delivery']}|{r['lang']}",
                         "t904-gen-gemma4-dual-label-v3", "gen", "delivery")
    supp_gen = f"{args.gen_dir}/gen_supp.jsonl"
    if os.path.exists(supp_gen):
        dual_qos += merge_gen(supp_gen,
                              f"{args.gen_dir}/label_supp_a.jsonl", f"{args.gen_dir}/label_supp_b.jsonl",
                              lambda r: f"{r['domain']}|{r['qos_family']}|{r['lang']}",
                              "t904-gen-supp-dual-label-v3", "gen_supp", "delivery")

    # ---- κ (qos_profile) no subconjunto dual ----
    if dual_qos:
        labs = sorted({q for p in dual_qos for q in p})
        idx = {q: i for i, q in enumerate(labs)}
        n = len(labs)
        mat = [[0] * n for _ in range(n)]
        for a, b in dual_qos:
            mat[idx[a]][idx[b]] += 1
        tot = len(dual_qos)
        po = sum(mat[i][i] for i in range(n)) / tot
        pe = sum(sum(mat[i]) * sum(mat[j][i] for j in range(n)) for i in range(n)) / (tot * tot)
        kappa = (po - pe) / (1 - pe) if pe < 1 else 1.0
        stats["kappa_qos"] = round(kappa, 4)
        stats["dual_n"] = tot

    # ---- C2: t901 (reused-reviewed, train-only) ----
    for r in load_jsonl(args.t901):
        exp = dict(r.get("expected") or {})
        if "qos_profile" not in exp:
            continue
        text = r["state"]["text"] if isinstance(r.get("state"), dict) else str(r.get("state"))
        rows.append(row(text, exp, "t901-" + hashlib.sha1(norm_text(text)[:120].encode()).hexdigest()[:10],
                        "reused-reviewed", "train-only", r.get("language", "en"),
                        "t901-contrastive", "t901-contrastive-reviewed"))
        stats["t901"] += 1

    # ---- B: replay sintético (train-only, rótulos completos) ----
    for r in load_jsonl(args.synthetic):
        exp = r.get("expected") or {}
        if "qos_profile" not in exp:
            continue
        text = r["state"]["text"] if isinstance(r.get("state"), dict) else str(r.get("state"))
        tags = r.get("tags") if isinstance(r.get("tags"), list) else []
        rows.append(row(text, exp, "syn-" + hashlib.sha1(norm_text(text)[:120].encode()).hexdigest()[:10],
                        "reused-synthetic", "train-only",
                        "pt" if any("pt" in str(t) for t in tags) else "en",
                        "synthetic-replay", "t900-calib-v3-synthetic-by-construction"))
        stats["synthetic"] += 1

    # ---- A: v4_train real (rótulo SÓ qos; subamostra balanceada) ----
    v4 = []
    for r in load_jsonl(args.v4train):
        exp = r.get("expected") or {}
        if exp.get("qos_profile") not in ("QI_LOW_COST", "QI_BALANCED", "QI_CRITICAL"):
            continue
        text = r["state"]["text"] if isinstance(r.get("state"), dict) else str(r.get("state"))
        v4.append((exp["qos_profile"], text, r.get("tags") if isinstance(r.get("tags"), list) else []))
    by_q: dict = {}
    for q, t, tags in v4:
        by_q.setdefault(q, []).append((t, tags))
    per = max(1, args.v4_max // len(by_q))
    for q, items in sorted(by_q.items()):
        rng.shuffle(items)
        for t, tags in items[:per]:
            rows.append(row(t, {"qos_profile": q},
                            "v4-" + hashlib.sha1(norm_text(t)[:120].encode()).hexdigest()[:10],
                            "reused-real", "train-only", "en", "sprout-pareto-measured",
                            "sprout-v4-train-seed900", {"orig_tags": [str(x) for x in tags[:4]]}))
            stats[f"v4_{q}"] += 1

    # ---- hard negatives determinísticos (train-only; rótulo por construção com sinal explícito) ----
    for r in load_jsonl(args.hardnegs):
        if r["stratum"].startswith("pair-STREAM"):
            qos = "QI_STREAM_LIKE"
        elif r["stratum"] == "deadline-not-qos":
            qos = "QI_BALANCED"
        else:
            qos = "QI_BALANCED"
        urg = 2 if ("ontem" in r["text"] or "2 hours" in r["text"]) else (
            0 if ("sem pressa" in r["text"] or "next month" in r["text"]) else 1)
        expected = {"qos_profile": qos, "needs_planning": False, "complexity": 1, "urgency": urg,
                    **{f"cap__{c}": False for c in CAPABILITY_CATALOG}}
        rows.append(row(r["text"], expected, r["family"], "template-hardneg", "train-only",
                        r["lang"], r["stratum"], "t904-deterministic-hardneg"))
        stats["hardneg"] += 1

    # ---- auditoria anti-vazamento: 8-gramas contra v4_test (PROIBIDO) e intra-pool ----
    stop_grams = set()
    for r in load_jsonl(args.v4test):
        t = r["state"]["text"] if isinstance(r.get("state"), dict) else str(r.get("state"))
        stop_grams |= ngrams(norm_text(t))
    leaked = []
    seen_norm = {}
    for i, r in enumerate(rows):
        t = norm_text(r["state"]["text"])
        if t in seen_norm:
            rows[i] = None
            stats["dedup_exact"] += 1
            continue
        seen_norm[t] = i
        g = ngrams(t)
        hits = g & stop_grams
        if hits and r["_meta"]["provenance"].startswith(("t904", "t901")):
            leaked.append((r["_meta"]["family"], len(hits)))
    rows = [r for r in rows if r is not None]
    stats["v4test_8gram_hits_newgen_or_t901"] = len(leaked)
    for fam, _ in leaked:  # zera qualquer família suspeita de treino/dev/sealed
        for r in rows:
            if r["_meta"]["family"] == fam and r["_meta"]["provenance"].startswith(("t904", "t901")):
                r["_meta"]["tier"] = "review-leak-suspect"
                r["_meta"]["partition_hint"] = "excluded"
                stats["leak_excluded"] += 1

    with open(args.out, "w") as f:
        for r in rows:
            f.write(json.dumps(r, ensure_ascii=False) + "\n")

    qos_dist = Counter(r["expected"]["qos_profile"] for r in rows if "qos_profile" in r["expected"])
    summary = {"rows": len(rows), "qos_dist": dict(qos_dist), "stats": dict(stats),
               "families_new_gen": len({r['_meta']['family'] for r in rows
                                        if r['_meta']['partition_hint'] == 'new-gen'})}
    with open(args.out + ".summary.json", "w") as f:
        json.dump(summary, f, indent=1, ensure_ascii=False)
    print(json.dumps(summary, indent=1, ensure_ascii=False))
    if stats.get("kappa_qos", 1.0) < 0.6:
        print("AVISO: kappa < 0.6 — investigar rubricas antes de treinar", file=sys.stderr)
    return 0


if __name__ == "__main__":
    sys.exit(main())
