#!/usr/bin/env python3
"""T912-F1b — Monta o dataset T912 (12 perguntas: 4 eixos + 8 caps individuais).

Fontes (TODAS em tree; gen novos entram via t912/datasets/raw/):
  A. pool_t910_items.jsonl (8.850 flat) — replay multitarefa
  B. t909 splits/train.jsonl (5.456 sdk 12q) — replay T908/T909
  C. t911 train_t911.jsonl (3.978) — replay T911
  D. NOVOS: raw/gen_{axis}.jsonl + raw/label_{axis}.jsonl (construção prevalece
     no eixo construído APÓS concordância com o rótulo LLM v6 — disciplina T910)

Regras:
  - dedup texto exato (prioridade D > A > B > C)
  - exclusão: texto exato OU overlap 8-gram vs os 7 sealeds históricos
    (t905/t906/t907/t908/t909/t910/t911)
  - split: dev 1000 / cal 700 (raras >=30) → train floors (qos raras >=150,
    cx3 >=150, urg1 >=250, caps positivas >=150 cada) → sealed 2000 (raras >=80,
    caps positivas >=60) → train resto com BALANCED <= 3000
  - formato de saída: {state:{text}, questions: Q12, gold:{12 chaves}}
  - FAIL-FAST (exit 2) se floors do train não forem atingidos (censo impresso)
  - MANIFEST.json com proveniência, censos e SHA-256
"""
import argparse
import hashlib
import json
import os
import random
import re
import sys
from collections import Counter, defaultdict

sys.path.insert(0, "/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/src")
from laya_worker.questions import QUESTIONS_V1, CAPABILITY_CATALOG

H = "/var/mnt/HD1TB/tese/src/laya_t900_handoff"
T912 = f"{H}/t912"
OUT = f"{T912}/datasets"
CAPS = list(CAPABILITY_CATALOG)
PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
RARE_QOS = ["QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
SEALED_HIST = [
    f"{H}/t905/datasets/sealed_export/sealed_t905_pool.jsonl",
    f"{H}/t906/datasets/sealed_export/sealed_t906_pool.jsonl",
    f"{H}/t907/datasets/sealed_export/sealed_t907_pool.jsonl",
    f"{H}/t908/datasets/sealed_export/sealed_t908_pool.jsonl",
    f"{H}/t909/datasets/sealed_t909/pool.jsonl",
    f"{H}/t909/datasets/sealed_t910/pool.jsonl",
    f"{H}/t911/datasets/sealed_t911.jsonl",
]

# floors/gates de dose (treino) e quotas (sealed/dev/cal)
TRAIN_FLOOR = {"qos_rare": 150, "cx3": 150, "urg1": 250, "cap_pos": 150}
DEV_N, CAL_N = 1000, 700
DEVCAL_RARE = 30
SEALED_N = 2000
SEALED_QUOTA_RARE = 80
SEALED_QUOTA_CAP = 60
BALANCED_CAP = 3000

Q12 = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
for c in CAPS:
    Q12[f"cap__{c}"] = {"type": "noul",
                        "instructions": f"Does this request require the capability '{c}'?"}


def load(p):
    out = []
    try:
        for l in open(p, errors="replace"):
            l = l.strip()
            if l:
                out.append(json.loads(l))
    except FileNotFoundError:
        pass
    return out


def ngrams(t, n=8):
    toks = re.findall(r"[a-z0-9]+", t.lower())
    return {" ".join(toks[i:i + n]) for i in range(len(toks) - n + 1)}


def flat(text, lang, prov, qos, plan, cx, urg, caps):
    return {"text": text, "lang": lang, "prov": prov, "qos": qos, "plan": bool(plan),
            "cx": int(cx), "urg": int(urg),
            "caps": {c: bool(caps.get(c)) for c in CAPS}}


def to_row(it):
    gold = {"qos_profile": it["qos"], "needs_planning": it["plan"],
            "complexity": it["cx"], "urgency": it["urg"]}
    gold.update({f"cap__{c}": it["caps"][c] for c in CAPS})
    return {"state": {"text": it["text"]}, "questions": Q12, "gold": gold}


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--census-only", action="store_true",
                    help="imprime censos das fontes sem escrever splits")
    a = ap.parse_args()
    rng = random.Random(912)

    # ---------- fontes ----------
    items = []
    prov_counts = Counter()

    # D. novos (gen + label com concordância)
    raw = f"{OUT}/raw"
    for ax in ("T912-QOS-RARE", "S-CX3", "S-URG", "S-CAPS8"):
        gens = {r["id"]: r for r in load(f"{raw}/gen_{ax}.jsonl") if r.get("text")}
        labs = {r["id"]: r["llm"] for r in load(f"{raw}/label_{ax}.jsonl")}
        kept = drop_disc = unlabeled = 0
        for rid, g in gens.items():
            lab = labs.get(rid)
            if not lab:
                unlabeled += 1
                continue
            ok = True
            if g["kind"] == "qos":
                ok = lab["qos_profile"] == g["gold"]
            elif g["kind"] == "cx":
                ok = int(lab["complexity"]) == int(g["gold"])
            elif g["kind"] == "urg":
                ok = int(lab["urgency"]) == int(g["gold"])
            elif g["kind"] == "cap" and g["gold"]:
                ok = all(lab["caps"].get(c) for c in g["gold"])
            if not ok:
                drop_disc += 1
                continue
            it = flat(g["text"], g["lang"], f"T912GEN_{ax}", lab["qos_profile"],
                      lab["needs_planning"], lab["complexity"], lab["urgency"], lab["caps"])
            # construção prevalece APENAS no eixo construído
            if g["kind"] == "qos":
                it["qos"] = g["gold"]
            elif g["kind"] == "cx":
                it["cx"] = int(g["gold"])
            elif g["kind"] == "urg":
                it["urg"] = int(g["gold"])
            items.append(it)
            kept += 1
        prov_counts[f"T912GEN_{ax}"] = kept
        if kept or drop_disc:
            print(f"[gen {ax}] kept={kept} drop_disc={drop_disc} unlabeled={unlabeled}")

    # A. pool
    for it in load(f"{H}/t909/datasets/pool_t910_items.jsonl"):
        items.append(flat(it["text"], it.get("lang", "?"), f"POOL_{it.get('prov','?')}",
                          it["qos"], it["plan"], it["cx"], it["urg"], it["caps"]))
        prov_counts[f"POOL_{it.get('prov','?')}"] += 1
    # B. splits T909 (sdk: expected + _meta)
    for r in load(f"{H}/t909/datasets/splits/train.jsonl"):
        e = r.get("expected") or {}
        if not all(k in e for k in ("qos_profile", "needs_planning", "complexity", "urgency")):
            continue
        items.append(flat(r["state"]["text"], (r.get("_meta") or {}).get("lang", "?"),
                          "REPLAY_t909_train", e["qos_profile"], e["needs_planning"],
                          e["complexity"], e["urgency"],
                          {c: e.get(f"cap__{c}", False) for c in CAPS}))
        prov_counts["REPLAY_t909_train"] += 1
    # C. t911 train (gold)
    for r in load(f"{H}/t911/datasets/train_t911.jsonl"):
        g = r["gold"]
        items.append(flat(r["state"]["text"], "?", "REPLAY_t911_train",
                          g["qos_profile"], g["needs_planning"], g["complexity"],
                          g["urgency"], {c: g.get(f"cap__{c}", False) for c in CAPS}))
        prov_counts["REPLAY_t911_train"] += 1

    print(f"[fontes] total bruto: {len(items)} (prov top: {prov_counts.most_common(8)})")

    # ---------- dedup ----------
    seen = set()
    uniq = []
    for it in items:
        h = hashlib.sha256(it["text"].encode()).hexdigest()
        if h in seen:
            continue
        seen.add(h)
        uniq.append(it)
    print(f"[dedup] {len(items)} -> {len(uniq)} (drops {len(items)-len(uniq)})")

    # ---------- exclusão vs sealeds históricos ----------
    sealed_texts = set()
    sealed_ng = set()
    for p in SEALED_HIST:
        for r in load(p):
            t = (r.get("state") or {}).get("text") or r.get("text") or ""
            if t:
                sealed_texts.add(hashlib.sha256(t.encode()).hexdigest())
                sealed_ng |= ngrams(t)
    clean = []
    dropped_exact = dropped_8g = 0
    for it in uniq:
        h = hashlib.sha256(it["text"].encode()).hexdigest()
        if h in sealed_texts:
            dropped_exact += 1
            continue
        if ngrams(it["text"]) & sealed_ng:
            dropped_8g += 1
            continue
        clean.append(it)
    print(f"[exclusao sealeds x7] exact={dropped_exact} 8gram={dropped_8g} -> resta {len(clean)}")

    # ---------- censos ----------
    def census(grp):
        c = {"n": len(grp), "qos": Counter(), "cx": Counter(), "urg": Counter(),
             "plan": Counter(), "caps_pos": Counter()}
        for it in grp:
            c["qos"][it["qos"]] += 1
            c["cx"][it["cx"]] += 1
            c["urg"][it["urg"]] += 1
            c["plan"][it["plan"]] += 1
            for cap in CAPS:
                if it["caps"][cap]:
                    c["caps_pos"][cap] += 1
        return c

    cen = census(clean)
    print(f"[censo clean] qos={dict(cen['qos'])}")
    print(f"[censo clean] cx={dict(cen['cx'])} urg={dict(cen['urg'])} plan={dict(cen['plan'])}")
    print(f"[censo clean] caps_pos={dict(cen['caps_pos'])}")
    if a.census_only:
        json.dump({"clean": cen, "prov": dict(prov_counts)},
                  open(f"{OUT}/CENSUS_INTERMEDIATE.json", "w"), indent=1)
        return

    # ---------- splits ----------
    used = set()
    idx = list(range(len(clean)))
    rng.shuffle(idx)
    # sealed/dev/cal preferem itens NOVOS (T912GEN, dominios frescos) => teste mais OOD
    idx_gen_first = sorted(idx, key=lambda i: 0 if clean[i]["prov"].startswith("T912GEN") else 1)

    def take_quota(target, quotas, order=None):
        """quotas: {key: n}; key conforme rarity_keys. Preenche até bater todas."""
        got = Counter()
        for i in (order or idx):
            if i in used:
                continue
            it = clean[i]
            keys = [f"qos:{it['qos']}", f"cx:{it['cx']}", f"urg:{it['urg']}"]
            if not it["plan"]:
                keys.append("plan:False")
            for cap in CAPS:
                if it["caps"][cap]:
                    keys.append(f"cap:{cap}")
            if any(got[k] < n for k, n in quotas.items() if k in keys):
                used.add(i)
                target.append(it)
                for k in keys:
                    got[k] += 1

    dev, cal = [], []
    take_quota(dev, {**{f"qos:{p}": DEVCAL_RARE for p in RARE_QOS},
                     "cx:3": DEVCAL_RARE, "urg:1": DEVCAL_RARE, "urg:2": DEVCAL_RARE,
                     **{f"cap:{c}": DEVCAL_RARE for c in CAPS}}, order=idx_gen_first)
    take_quota(cal, {**{f"qos:{p}": DEVCAL_RARE for p in RARE_QOS},
                     "cx:3": DEVCAL_RARE, "urg:1": DEVCAL_RARE, "urg:2": DEVCAL_RARE,
                     **{f"cap:{c}": DEVCAL_RARE for c in CAPS}}, order=idx_gen_first)

    train = []
    # floors do treino: raras + cx3 + urg1 + caps (por item conta em todas as chaves)
    take_quota(train, {**{f"qos:{p}": TRAIN_FLOOR["qos_rare"] for p in RARE_QOS},
                       "cx:3": TRAIN_FLOOR["cx3"], "urg:1": TRAIN_FLOOR["urg1"],
                       **{f"cap:{c}": TRAIN_FLOOR["cap_pos"] for c in CAPS}})
    ctrain = census(train)
    print(f"[train floors] qos raras: "
          + ", ".join(f"{p}={ctrain['qos'][p]}" for p in RARE_QOS)
          + f" | cx3={ctrain['cx'][3]} urg1={ctrain['urg'][1]}"
          + f" | caps_pos={dict(ctrain['caps_pos'])}")
    fail = []
    for p in RARE_QOS:
        if ctrain["qos"][p] < TRAIN_FLOOR["qos_rare"]:
            fail.append(f"qos {p}={ctrain['qos'][p]}<{TRAIN_FLOOR['qos_rare']}")
    if ctrain["cx"][3] < TRAIN_FLOOR["cx3"]:
        fail.append(f"cx3={ctrain['cx'][3]}<{TRAIN_FLOOR['cx3']}")
    if ctrain["urg"][1] < TRAIN_FLOOR["urg1"]:
        fail.append(f"urg1={ctrain['urg'][1]}<{TRAIN_FLOOR['urg1']}")
    for c in CAPS:
        if ctrain["caps_pos"][c] < TRAIN_FLOOR["cap_pos"]:
            fail.append(f"cap {c}={ctrain['caps_pos'][c]}<{TRAIN_FLOOR['cap_pos']}")
    if fail:
        print("[FAIL-FAST] floors nao atingidos: " + "; ".join(fail))
        json.dump({"fail": fail, "train_census": {k: dict(v) for k, v in ctrain.items()}},
                  open(f"{OUT}/CENSUS_FAIL.json", "w"), indent=1)
        sys.exit(2)

    sealed = []
    take_quota(sealed, {**{f"qos:{p}": SEALED_QUOTA_RARE for p in RARE_QOS},
                        "cx:3": SEALED_QUOTA_RARE, "urg:1": SEALED_QUOTA_RARE,
                        "urg:2": SEALED_QUOTA_RARE, "plan:False": 300,
                        **{f"cap:{c}": SEALED_QUOTA_CAP for c in CAPS}}, order=idx_gen_first)
    # completa sealed até 2000: primeiro novos (gen), depois mistura natural
    for i in idx_gen_first:
        if len(sealed) >= SEALED_N:
            break
        if i not in used and clean[i]["prov"].startswith("T912GEN"):
            used.add(i)
            sealed.append(clean[i])
    for i in idx:
        if len(sealed) >= SEALED_N:
            break
        if i not in used:
            used.add(i)
            sealed.append(clean[i])
    # completa dev/cal
    for i in idx:
        if len(dev) >= DEV_N:
            break
        if i not in used:
            used.add(i)
            dev.append(clean[i])
    for i in idx:
        if len(cal) >= CAL_N:
            break
        if i not in used:
            used.add(i)
            cal.append(clean[i])
    # train: resto com teto BALANCED
    for i in idx:
        if i in used:
            continue
        it = clean[i]
        if it["qos"] == "QI_BALANCED" and ctrain["qos"]["QI_BALANCED"] >= BALANCED_CAP:
            continue
        used.add(i)
        train.append(it)
        ctrain["qos"][it["qos"]] += 1

    # ---------- export ----------
    os.makedirs(OUT, exist_ok=True)
    for name, grp in (("train", train), ("dev", dev), ("cal", cal)):
        with open(f"{OUT}/{name}_t912.jsonl", "w") as f:
            for it in grp:
                f.write(json.dumps(to_row(it), ensure_ascii=False) + "\n")
    os.makedirs(f"{T912}/sealed", exist_ok=True)
    with open(f"{T912}/sealed/pool.jsonl", "w") as f:
        for i, it in enumerate(sealed):
            r = {"state": {"text": it["text"]}, "questions": {}, "_row_id": f"sealed:{i}"}
            f.write(json.dumps(r, ensure_ascii=False) + "\n")
    labels = {f"sealed:{i}": to_row(it)["gold"] for i, it in enumerate(sealed)}
    with open(f"{T912}/sealed/labels.json", "w") as f:
        json.dump(labels, f, indent=1)
    import time
    proto = {"campaign": "T912", "status": "NOT_OPENED", "open_count": 0, "n": len(sealed),
             "sha256_pool": hashlib.sha256(open(f"{T912}/sealed/pool.jsonl", "rb").read()).hexdigest(),
             "sha256_labels": hashlib.sha256(open(f"{T912}/sealed/labels.json", "rb").read()).hexdigest(),
             "created": time.strftime("%Y-%m-%d"),
             "rule": "abertura UNICA apos freeze pesos+calibrador; leak 8-gram vs 7 sealeds historicos"}
    json.dump(proto, open(f"{T912}/sealed/SEALED_OPEN_PROTOCOL.json", "w"), indent=1)

    manifest = {"generated": time.strftime("%Y-%m-%dT%H:%M:%S"),
                "sources": dict(prov_counts),
                "counts": {"raw": len(items), "uniq": len(uniq),
                           "dropped_exact": dropped_exact, "dropped_8gram": dropped_8g,
                           "clean": len(clean)},
                "splits": {n: census(g) for n, g in (("train", train), ("dev", dev),
                                                     ("cal", cal), ("sealed", sealed))},
                "train_floors": TRAIN_FLOOR, "balanced_cap": BALANCED_CAP,
                "files": {}}
    for fn in (f"{OUT}/train_t912.jsonl", f"{OUT}/dev_t912.jsonl", f"{OUT}/cal_t912.jsonl",
               f"{T912}/sealed/pool.jsonl", f"{T912}/sealed/labels.json"):
        manifest["files"][os.path.basename(fn)] = {
            "sha256": hashlib.sha256(open(fn, "rb").read()).hexdigest(),
            "lines": sum(1 for _ in open(fn))}
    json.dump(manifest, open(f"{OUT}/MANIFEST.json", "w"), indent=1)
    for n, g in (("train", train), ("dev", dev), ("cal", cal), ("sealed", sealed)):
        c = census(g)
        print(f"[{n.upper()}] n={c['n']} qos={dict(c['qos'])} cx3={c['cx'][3]} "
              f"urg1={c['urg'][1]} caps_pos={dict(c['caps_pos'])}")
    print("OK -> datasets t912")


if __name__ == "__main__":
    main()
