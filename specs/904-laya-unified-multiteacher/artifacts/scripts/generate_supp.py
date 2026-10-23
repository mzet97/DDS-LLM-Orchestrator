#!/usr/bin/env python3
"""T904-M1 suplemento — textos direcionados para classes QoS sub-representadas
(QI_FAILOVER, QI_LOW_COST, QI_CRITICAL) com o MESMO pipeline (gen 6/lote sem rótulo
+ label A no-think + label B 12%). Reaproveita chat/label de generate_contrastive.
"""
import json
import os
import random
import sys
import time

sys.path.insert(0, os.path.dirname(os.path.abspath(__file__)))
import generate_contrastive as G  # noqa: E402

DOMAINS2 = G.DOMAINS
LANGS = G.LANGS
PERSONAS = G.PERSONAS

SCENARIOS = [
    ("failover", "the workload must KEEP RUNNING even when execution nodes/services become unavailable: describe retries, failover to a secondary region/node, degraded-mode continuity"),
    ("failover", "mention recent outages/unreliable infrastructure and require the system to survive them automatically (retry/failover/continuity), single final report"),
    ("lowcost", "the priority is minimal resource/cost: batch processing off-peak, no latency sensitivity, cheapest acceptable quality"),
    ("lowcost", "huge backlog, run it cheaply overnight, throughput over speed, no deadline"),
    ("critical", "hard external deadline within hours and reliability-sensitive: payments/production/incident; must not miss the deadline"),
    ("critical", "SLA-bound operation where missing the window has business consequences; deadline stated explicitly"),
]


def main() -> None:
    out_dir = "/tmp/laya_t904_gen"
    os.makedirs(out_dir, exist_ok=True)
    gen_path = f"{out_dir}/gen_supp.jsonl"
    rng = random.Random(905)
    grid = [(s, d, l) for s in SCENARIOS for d in DOMAINS2 for l in LANGS]
    rng.shuffle(grid)
    target_batches = 70
    done = set()
    if os.path.exists(gen_path):
        with open(gen_path) as f:
            for line in f:
                try:
                    r = json.loads(line)
                    if "text" in r:
                        done.add(r["batch"])
                except Exception:
                    pass
    i = 0
    t0 = time.time()
    while len(done) < target_batches and i < len(grid):
        (qos_family, spec), domain, (lang, lang_desc) = grid[i]
        i += 1
        bid = f"s{len(done):03d}"
        if bid in done:
            continue
        persona = rng.choice(PERSONAS)
        prompt = (
            f"You produce REALISTIC workflow requests for a software agent. Scenario (internal spec, do NOT mention it):\n"
            f"- domain: {domain}\n- requirement: {spec}\n"
            f"- written by: {persona}, language: {lang_desc}\n\n"
            f"Write 6 DIFFERENT natural requests (2-4 sentences each). Vary phrasing strongly; no meta commentary. "
            f'Return ONLY a JSON array: [{{"text": "...", "evidence": "<paraphrase, max 12 words, no quotes>"}}, ...].'
        )
        try:
            arr = G.lenient_json(G.chat(prompt, 0.95))
            items = arr if isinstance(arr, list) else []
        except Exception as e:  # noqa: BLE001
            with open(gen_path, "a") as f:
                f.write(json.dumps({"id": bid, "error": str(e)[:160]}) + "\n")
            done.add(bid)
            continue
        for k, it in enumerate(items[:6]):
            if isinstance(it, dict) and it.get("text"):
                with open(gen_path, "a") as f:
                    f.write(json.dumps({"id": f"{bid}-{k}", "batch": bid, "domain": domain,
                                        "qos_family": qos_family, "lang": lang,
                                        "text": it["text"][:1200],
                                        "gen_evidence": str(it.get("evidence", ""))[:200]},
                                       ensure_ascii=False) + "\n")
        done.add(bid)
    print(f"[GEN_SUPP done] {len(done)} batches in {time.time()-t0:.0f}s", flush=True)
    G.label_all(gen_path, f"{out_dir}/label_supp_a.jsonl", style=0, temp=0.0)
    print("[LABEL_SUPP_A done]", flush=True)
    rng2 = random.Random(906)
    by_stratum = {}
    with open(gen_path) as f:
        for line in f:
            r = json.loads(line)
            if "text" in r:
                by_stratum.setdefault((r.get("qos_family"), r.get("lang")), []).append(r["id"])
    subset = set()
    for key, ids in sorted(by_stratum.items()):
        k = max(1, round(len(ids) * 0.12))
        subset.update(rng2.sample(sorted(ids), k))
    tmp = f"{out_dir}/gen_supp_subset.jsonl"
    with open(gen_path) as f, open(tmp, "w") as g:
        for line in f:
            r = json.loads(line)
            if r.get("id") in subset:
                g.write(line)
    G.label_all(tmp, f"{out_dir}/label_supp_b.jsonl", style=1, temp=0.2)
    print("[ALL_SUPP done]", flush=True)


if __name__ == "__main__":
    main()
