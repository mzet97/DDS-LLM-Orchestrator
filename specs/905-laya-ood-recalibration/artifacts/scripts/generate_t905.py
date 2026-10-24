#!/usr/bin/env python3
"""T905-M1 — gerador com rótulos MEDIDOS por construção (roda na .61, stdlib-only).

Diferenças para o T904 (que era dual-LLM em tudo):
- complexity OURO = bucket do nº de estágios do cenário (1→0, 2-3→1, 4-6→2, 7+→3);
  o texto é construído para descrever EXATAMENTE esses estágios;
- urgency OURO = bucket do prazo do banco (frases com distância temporal inequívoca);
- needs_planning OURO = K>1; caps OURO = as capabilities exigidas pelo cenário;
- qos_profile = fronteira semântica → permanece por anotação LLM dupla + adjudicação
  (política v3), como na T904.
O LLM NUNCA define o gabarito medido; ele só verifica (label_b em 12% p/ κ).
"""
import argparse
import itertools
import json
import os
import random
import time
import urllib.request

MODEL = "/home/agent/models/gemma-4-12b-it-UD-Q4_K_XL.gguf"
URL = "http://127.0.0.1:8083/v1/chat/completions"
MAXTOK = 3000

QOS_CRITERIA = {
    "QI_BALANCED": "balanced latency and resource use",
    "QI_CRITICAL": "deadline and reliability-sensitive work",
    "QI_STREAM_LIKE": "interactive incremental streaming",
    "QI_LOW_COST": "resource-efficient batch work",
    "QI_FAILOVER": "resilience when execution resources are unavailable",
}
POLICY = """Labeling policy v3 (binding):
- QI_STREAM_LIKE ONLY when the user asks to RECEIVE usable intermediate results AS THEY ARE PRODUCED. Merely mentioning streaming topics or words like fast/parallel/async/real-time WITHOUT asking for partial delivery is NOT stream.
- QI_BALANCED: run and deliver the complete result at the end (progress bars/parallelism don't count).
- QI_LOW_COST: resource-efficient batch, no latency sensitivity. QI_CRITICAL: hard deadline/reliability. QI_FAILOVER: resilience to unavailable resources/retries.
- urgency: ONLY temporal evidence; ambiguity => 1. qos is NEVER derived from the deadline word alone."""

DOMAINS = ["robotics fleet control", "fintech compliance reports", "hospital records administration",
           "indie game asset pipeline", "automotive diagnostics", "telecom network audits",
           "legal contract analysis", "university grading workflows", "energy grid monitoring",
           "supply chain forecasting", "kubernetes cluster ops", "mobile app release trains",
           "blockchain analytics", "AR/VR content builds", "weather data pipelines",
           "genomics batch scoring", "speech synthesis QA", "CDN cache infrastructure",
           "chat infrastructure migration", "GIS route optimization", "library digitization",
           "restaurant supply inventory"]
PERSONAS = ["a platform engineer", "an operations lead", "a technical program manager",
            "a data engineer", "a QA lead", "um engenheiro de software", "uma coordenadora de TI",
            "um analista sênior"]
DEADLINES = [
    (2, "must be ready in the next 2 hours"),
    (2, "é para ontem — preciso em 1 hora para uma reunião"),
    (2, "needed immediately; the client is waiting on the line"),
    (1, "by the end of this week"),
    (1, "até sexta-feira, sem pressa excessiva"),
    (1, "in about three days"),
    (0, "next month, no rush at all"),
    (0, "quando houver disponibilidade na fila"),
    (0, "no deadline mentioned; process it eventually"),
]
DELIVERY = [
    ("stream", "ask to receive each stage's output as soon as it completes (partial deliverables)"),
    ("stream", "ask for continuous partial results while processing continues"),
    ("balanced", "ask to deliver ONLY the complete result at the end"),
    ("balanced", "ask to run everything in parallel and reply once, at the end"),
    ("balanced", "mention a progress bar but require a single final deliverable"),
    ("balanced", "explicitly forbid partial outputs ('do not send parts')"),
    ("balanced", "mention {tech} as the SUBJECT but require one final report"),
    ("lowcost", "ask to run it cheaply off-peak on spare capacity, throughput over speed"),
    ("failover", "require the pipeline to survive node/provider outages with automatic retry/failover"),
    ("failover", "mention recent outages and demand degraded-mode continuity until recovery"),
    ("critical", "hard deadline within hours plus reliability constraints (production/SLA)"),
    ("critical", "SLA-bound operation where missing the window has real business impact"),
]
TECHS = ["Kafka Streams", "SSE endpoints", "stdout pipelines", "video streaming jobs", "Redis Streams"]
CAPS_BANK = [[], [], ["research"], ["summarization"], ["translation"], ["unit_testing"],
             ["security_audit"], ["code_review"], ["research", "summarization"],
             ["unit_testing", "code_review"], ["security_audit", "translation"],
             ["research", "unit_testing"], ["summarization", "translation", "code_review"]]
LANGS = [("pt", "Portuguese (Brazil) with natural technical terms"), ("en", "English")]
STAGE_VERBS = ["collect", "clean", "transform", "analyze", "summarize", "validate",
               "package", "deploy", "report", "audit", "translate", "review"]


def stages_for(bucket: int, rng: random.Random):
    if bucket == 0:
        k = 1
    elif bucket == 1:
        k = rng.choice([2, 3])
    elif bucket == 2:
        k = rng.choice([4, 5, 6])
    else:
        k = rng.choice([7, 8, 9])
    return rng.sample(STAGE_VERBS, k)


def chat(prompt: str, temperature: float, retries: int = 4, maxtok: int = MAXTOK,
         no_think: bool = False) -> str:
    payload = {"model": MODEL, "messages": [{"role": "user", "content": prompt}],
               "max_tokens": maxtok, "temperature": temperature}
    if no_think:
        payload["chat_template_kwargs"] = {"enable_thinking": False}
    body = json.dumps(payload).encode()
    last = None
    for i in range(retries):
        try:
            req = urllib.request.Request(URL, data=body, headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=300) as r:
                out = json.loads(r.read())
            content = out["choices"][0]["message"]["content"] or ""
            if content.strip():
                return content
            last = RuntimeError("content vazio")
        except Exception as e:  # noqa: BLE001
            last = e
        time.sleep(3 * (i + 1))
    raise RuntimeError(f"chat falhou após {retries}: {last}")


def strip_fence(s: str) -> str:
    s = s.strip()
    if s.startswith("```"):
        s = s.split("```")[1]
        if s.startswith("json"):
            s = s[4:]
    return s.strip()


def lenient_json(raw: str):
    import re
    s = strip_fence(raw)
    for c in (s, re.sub(r",\s*([\]}])", r"\1", s)):
        for t in (c, c + '"', c + '"}]', c + "]}", c + "}"):
            try:
                return json.loads(t, strict=False)
            except Exception:
                continue
    try:
        obj, _ = json.JSONDecoder().raw_decode(s.strip().lstrip("["), strict=False)
        return obj if isinstance(obj, (dict, list)) else None
    except Exception:
        return None


def append(path, obj):
    with open(path, "a") as f:
        f.write(json.dumps(obj, ensure_ascii=False) + "\n")


def done_batches(path):
    done = set()
    if os.path.exists(path):
        with open(path) as f:
            for line in f:
                try:
                    r = json.loads(line)
                    if "text" in r and r.get("batch"):
                        done.add(r["batch"])
                except Exception:
                    continue
    return done


def cx_bucket(k: int) -> int:
    return 0 if k == 1 else 1 if k <= 3 else 2 if k <= 6 else 3


def gen(target: int, out: str, seed: int) -> None:
    rng = random.Random(seed)
    grid = list(itertools.product(DOMAINS, DELIVERY, DEADLINES, [0, 1, 2, 3], CAPS_BANK, LANGS))
    rng.shuffle(grid)
    i = 0
    while len(done_batches(out)) < target and i < len(grid):
        domain, (delivery, dspec), (urg_gold, deadline), cx_b, caps_gold, (lang, ldesc) = grid[i]
        i += 1
        bid = f"t{len(done_batches(out)):04d}"
        if bid in done_batches(out):
            continue
        stages = stages_for(cx_b, rng)
        k = len(stages)
        persona = rng.choice(PERSONAS)
        delivery_s = dspec.format(tech=rng.choice(TECHS))
        prompt = (
            f"You produce REALISTIC workflow requests for a software agent. Scenario (internal spec, do NOT mention it as a list):\n"
            f"- domain: {domain}\n- delivery expectation: {delivery_s}\n"
            f"- deadline: {deadline}\n"
            f"- the request must describe a pipeline with EXACTLY {k} stage(s), in this order: {', '.join(stages)}\n"
            f"- capability requirements to mention naturally: {', '.join(caps_gold) if caps_gold else 'none beyond coding'}\n"
            f"- written by: {persona}, language: {ldesc}\n\n"
            f"Write 6 DIFFERENT natural requests (2-4 sentences each; may blend the stages into flowing prose, "
            f"but ALL {k} stages must be recognizable and in order). Vary phrasing strongly; no meta commentary. "
            f'Return ONLY a JSON array: [{{"text": "...", "evidence": "<paraphrase, max 12 words, no quotes>"}}, ...].'
        )
        try:
            arr = lenient_json(chat(prompt, 0.95))
            items = arr if isinstance(arr, list) else []
        except Exception as e:  # noqa: BLE001
            append(out, {"id": bid, "error": str(e)[:160]})
            continue
        for j, it in enumerate(items[:6]):
            if isinstance(it, dict) and it.get("text"):
                append(out, {"id": f"{bid}-{j}", "batch": bid, "domain": domain,
                             "delivery": delivery, "lang": lang,
                             "k_stages": k, "complexity_gold": cx_bucket(k),
                             "needs_planning_gold": k > 1,
                             "urgency_gold": urg_gold, "deadline": deadline,
                             "caps_gold": caps_gold, "stages": stages,
                             "text": it["text"][:1200],
                             "gen_evidence": str(it.get("evidence", ""))[:200]})
    print(f"[GEN done] {len(done_batches(out))} batches in {time.time()-T0:.0f}s", flush=True)


def parse_label(raw: str):
    d = lenient_json(raw)
    return d if isinstance(d, list) else None


def label_all(src: str, out: str, style: int, temp: float, chunk: int = 5) -> None:
    seen = set()
    if os.path.exists(out):
        with open(out) as f:
            for line in f:
                try:
                    seen.add(json.loads(line)["id"])
                except Exception:
                    continue
    rows = []
    with open(src) as f:
        for line in f:
            r = json.loads(line)
            if "text" in r and f"{r['id']}:s{style}" not in seen:
                rows.append(r)
    caps = ["rust_refactor", "python_refactor", "unit_testing", "security_audit",
            "code_review", "research", "summarization", "translation"]
    for i in range(0, len(rows), chunk):
        part = rows[i:i + chunk]
        texts = "\n\n".join(f"[{r['id']}] \"\"\"{r['text']}\"\"\"" for r in part)
        prompt = (
            f"{POLICY}\n\nQuestions:\n"
            f"qos_profile (choice): Which QoS objective best fits this workload?\n" +
            "\n".join(f"  - {k}: {v}" for k, v in QOS_CRITERIA.items()) + "\n"
            f"needs_planning (bool): Does the request require multiple dependent tasks?\n"
            f"complexity (0-3), urgency (0-2), caps (8 bools: {', '.join(caps)}).\n\n"
            f"Label EACH numbered request INDEPENDENTLY. Evidence = short paraphrase (max 12 words, NO quotes inside).\n"
            f'Return ONLY a JSON array: [{{"id": "...", "qos_profile": "...", "needs_planning": bool, '
            f'"complexity": 0-3, "urgency": 0-2, "caps": {{...}}, "evidence": "..."}}, ...]\n\n{texts}'
        )
        raw = chat(prompt, temp, maxtok=3000, no_think=True)
        arr = lenient_json(raw)
        items = arr if isinstance(arr, list) else []
        if not items:
            append(out, {"id": f"chunk{i}:s{style}", "error": raw[-140:]})
            continue
        for it in items:
            if not isinstance(it, dict) or "id" not in it:
                continue
            lab = dict(it.get("label", it))
            lab.pop("id", None)
            try:
                lab["complexity"] = max(0, min(3, int(lab.get("complexity", 1))))
                lab["urgency"] = max(0, min(2, int(lab.get("urgency", 1))))
                lab["needs_planning"] = bool(lab.get("needs_planning"))
                lab["caps"] = {c: bool(lab.get("caps", {}).get(c, False)) for c in caps}
            except Exception:
                continue
            if lab.get("qos_profile") in QOS_CRITERIA:
                append(out, {"id": f"{it['id']}:s{style}", "label": lab})


def main() -> None:
    global T0
    T0 = time.time()
    ap = argparse.ArgumentParser()
    ap.add_argument("--target", type=int, default=360, help="batches de 6 textos")
    ap.add_argument("--out", default="/tmp/laya_t905_gen")
    ap.add_argument("--seed", type=int, default=905)
    ap.add_argument("--label-b-frac", type=float, default=0.12)
    ap.add_argument("--phase", choices=["gen", "label", "all"], default="all")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    genp = f"{args.out}/gen.jsonl"
    if args.phase in ("gen", "all"):
        gen(args.target, genp, args.seed)
    if args.phase == "gen":
        print("[gen-only]", flush=True)
        return
    label_all(genp, f"{args.out}/label_a.jsonl", style=0, temp=0.0)
    print(f"[LABEL_A done] {time.time()-T0:.0f}s", flush=True)
    rng = random.Random(args.seed + 7)
    strat = {}
    with open(genp) as f:
        for line in f:
            r = json.loads(line)
            if "text" in r:
                strat.setdefault((r.get("delivery"), r.get("lang")), []).append(r["id"])
    subset = set()
    for key, ids in sorted(strat.items()):
        k = max(1, round(len(ids) * args.label_b_frac))
        subset.update(rng.sample(sorted(ids), k))
    tmp = f"{args.out}/gen_subset_b.jsonl"
    with open(genp) as f, open(tmp, "w") as g:
        for line in f:
            r = json.loads(line)
            if r.get("id") in subset:
                g.write(line)
    label_all(tmp, f"{args.out}/label_b.jsonl", style=1, temp=0.2)
    print(f"[ALL done] {time.time()-T0:.0f}s", flush=True)


if __name__ == "__main__":
    main()
