#!/usr/bin/env python3
"""T904-M1 — gerador + dupla anotação de candidatos contrastivos (roda NA .61, stdlib-only).

Evidência-antes-de-rótulo: fase GEN produz TEXTOS por cenário (sem rótulo prévio);
fases LABEL_A/LABEL_B rotulam independentemente com rubricas redigidas diferente
(mesma política v3); concordância total em qos_profile (e demais qids) => tier
silver; divergência => status review (adjudicação local depois). HTTP sequencial
(server -np 1: 2 clientes = GPU hang). Retomável por id. Sem segredos em logs.

Uso na .61:  python3 generate_contrastive.py --target-gen 1400 --out /tmp/laya_t904_gen
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
MAXTOK = 3000  # <1024 => content vazio (reasoning consome); 3 textos + evidência exige folga

# ---- QUESTIONS_V1 (texto EXATO de questions.py rev 1 — não reescrever) ----
QOS_CRITERIA = {
    "QI_BALANCED": "balanced latency and resource use",
    "QI_CRITICAL": "deadline and reliability-sensitive work",
    "QI_STREAM_LIKE": "interactive incremental streaming",
    "QI_LOW_COST": "resource-efficient batch work",
    "QI_FAILOVER": "resilience when execution resources are unavailable",
}
QOS_INSTR = "Which QoS objective best fits this workload?"
NEEDS_INSTR = "Does the request require multiple dependent tasks?"
COMPLEX_INSTR = "How complex is the task?"
COMPLEX_LEVELS = ["simple", "moderate", "complex", "very_complex"]
URGENCY_INSTR = "How urgent is the task?"
URGENCY_LEVELS = ["low", "normal", "high"]
CAPS = ["rust_refactor", "python_refactor", "unit_testing", "security_audit",
        "code_review", "research", "summarization", "translation"]

POLICY = """Labeling policy v3 (binding):
- QI_STREAM_LIKE ONLY when the user asks to RECEIVE usable intermediate results AS THEY ARE PRODUCED (partial/continuous deliverables). Merely mentioning streaming topics (Kafka Streams, SSE, video streaming, stdout, log streams) or words like fast/parallel/async/real-time WITHOUT asking for partial delivery is NOT stream.
- QI_BALANCED: run and deliver the complete result at the end; progress bars or parallelism do not make it stream. "Do not send parts" explicitly = BALANCED (unless another dominant requirement).
- QI_LOW_COST: resource-efficient batch, no latency sensitivity.
- QI_CRITICAL: hard deadline/reliability-sensitive work (deadline affects urgency, not automatically qos).
- QI_FAILOVER: resilience/retry when execution resources may be unavailable.
- urgency: ONLY temporal evidence in the text (explicit deadlines, ASAP, "no rush"). Ambiguous deadline => urgency=1.
- complexity: 0 simple/1 moderate/2 complex/3 very_complex by number of dependent stages and breadth.
- needs_planning: true iff multiple dependent tasks/stages.
- caps: mark ONLY capabilities the text actually requires."""

DOMAINS = ["e-commerce pricing", "log observability", "document translation", "video transcode pipeline",
           "ML model training", "ETL data pipelines", "web scraping", "financial reports", "IoT sensor ingestion",
           "support chatbot", "search indexing", "backup/restore", "CI/CD test triage", "docs generation",
           "audio transcription", "geospatial processing", "PDF OCR extraction", "fraude em transações",
           "sistema de recomendação", "monitoramento de redes sociais", "planilhas contábeis",
           "integração de APIs legadas", "análise de churn", "compressão de imagens"]
PERSONAS = ["a backend developer", "a data analyst", "an SRE", "a product manager", "a QA engineer",
            "um desenvolvedor", "uma analista de dados", "um engenheiro de platforma"]
DELIVERY = [
    ("stream", "ask to receive each partial result as soon as it is ready (incremental delivery)"),
    ("stream", "ask for continuous partial outputs while processing continues"),
    ("balanced", "ask to deliver ONLY the complete result at the end"),
    ("balanced", "mention showing a progress percentage but delivering the final report at the end"),
    ("balanced", "ask to run steps in parallel and answer only when everything finishes"),
    ("balanced", "explicitly forbid sending partial versions ('do not send parts')"),
    ("balanced", "mention Kafka Streams / SSE / stream processing as the SUBJECT of the work, but require a single final deliverable"),
    ("balanced", "ask for it to be fast/asynchronous/real-time WITHOUT partial delivery"),
    ("ambig", "be genuinely ambiguous about whether partial results are wanted"),
]
DEADLINES = ["must finish today in 2 hours", "needed by end of week, no rush", "no deadline mentioned",
             "ASAP, production is down", "flexible timing this month", "deadline tomorrow morning"]
COMPLEXITY = ["single straightforward step", "pipeline with a few dependent stages",
              "multi-domain project with research, implementation and validation", "large very complex program with many interdependent parts"]
CAPSHINTS = ["also requires translating outputs", "also requires summarizing findings", "no extra capabilities",
             "also requires a security audit", "also requires unit tests", "also requires prior web research",
             "also requires refactoring legacy python code", "also requires a code review of the patch"]

LANGS = [("pt", "Portuguese (Brazil) with technical terms in English where natural"),
         ("en", "English")]


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
            last = RuntimeError("content vazio (reasoning consumiu o orçamento)")
        except Exception as e:  # noqa: BLE001 — log e retry
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


def done_ids(path: str) -> set:
    ids = set()
    if os.path.exists(path):
        with open(path) as f:
            for line in f:
                try:
                    ids.add(json.loads(line)["id"])
                except Exception:
                    continue
    return ids


def append(path: str, obj) -> None:
    with open(path, "a") as f:
        f.write(json.dumps(obj, ensure_ascii=False) + "\n")


def done_batches(out: str) -> set:
    """Batches com textos reais (linhas de erro são re-tentáveis)."""
    done = set()
    if os.path.exists(out):
        with open(out) as f:
            for line in f:
                try:
                    r = json.loads(line)
                except Exception:
                    continue
                if "text" in r and r.get("batch"):
                    done.add(r["batch"])
    return done


def gen_texts(n_batches: int, out: str, seed: int) -> None:
    rng = random.Random(seed)
    seen = done_ids(out)
    grid = list(itertools.product(DOMAINS, DELIVERY, DEADLINES, COMPLEXITY, CAPSHINTS, LANGS))
    rng.shuffle(grid)
    i = 0
    while len(done_batches(out)) < n_batches and i < len(grid):
        domain, (delivery, delivery_desc), deadline, cx, caphint, (lang, lang_desc) = grid[i]
        i += 1
        bid = f"g{len(seen):05d}"
        if bid in seen:
            continue
        persona = rng.choice(PERSONAS)
        prompt = (
            f"You produce REALISTIC workflow requests for a software agent. Scenario (internal spec, do NOT mention it):\n"
            f"- domain: {domain}\n- delivery expectation: {delivery_desc}\n"
            f"- deadline: {deadline}\n- complexity: {cx}\n- extras: {caphint}\n"
            f"- written by: {persona}, language: {lang_desc}\n\n"
            f"Write 6 DIFFERENT natural requests (2-4 sentences each, may mix technical terms). "
            f"Vary phrasing/vocabulary strongly; do NOT reuse stock phrases across the 3; no meta commentary. "
            f'Return ONLY a JSON array: [{{"text": "...", "evidence": "<short excerpt that decides the delivery expectation>"}}, ...].'
        )
        try:
            raw = chat(prompt, 0.95)
            arr = json.loads(strip_fence(raw))
        except Exception as e:  # noqa: BLE001
            append(out, {"id": bid, "error": str(e)[:200]})
            seen.add(bid)
            continue
        items = arr if isinstance(arr, list) else []
        for k, it in enumerate(items[:6]):
            if not isinstance(it, dict) or not it.get("text"):
                continue
            append(out, {"id": f"{bid}-{k}", "batch": bid, "domain": domain, "delivery": delivery,
                         "deadline": deadline, "complexity_hint": cx, "caps_hint": caphint,
                         "lang": lang, "persona": persona, "text": it["text"][:1200],
                         "gen_evidence": str(it.get("evidence", ""))[:300]})
        seen.add(bid)


def _label_prompt(style: int, text: str) -> str:
    qos = "\n".join(f"  - {k}: {v}" for k, v in QOS_CRITERIA.items())
    caps = ", ".join(CAPS)
    rubric = POLICY if style == 0 else POLICY.replace(
        "Labeling policy v3 (binding):", "Decision rubric v3 (authoritative):").replace(
        "ONLY when the user asks", "exclusively when the user explicitly requests").replace(
        "ASAP, production is down", "immediate deadlines")
    return (
        f"{rubric}\n\nQuestions (answer ALL, use EXACTLY these semantics):\n"
        f"qos_profile (choice): {QOS_INSTR}\n{qos}\n"
        f"needs_planning (bool): {NEEDS_INSTR}\n"
        f"complexity (0-3): {COMPLEX_INSTR} levels {COMPLEX_LEVELS}\n"
        f"urgency (0-2): {URGENCY_INSTR} levels {URGENCY_LEVELS}\n"
        f"caps (8 bools): for each of [{caps}]: {NEEDS_INSTR.replace('multiple dependent tasks', 'this capability')}\n\n"
        f"REQUEST:\n\"\"\"{text}\"\"\"\n\n"
        f'Return ONLY JSON: {{"qos_profile": "...", "needs_planning": true/false, "complexity": 0-3, '
        f'"urgency": 0-2, "caps": {{"rust_refactor": bool, ...}}, "evidence": "<<=200 chars quote>", '
        f'"confidence": 0.0-1.0}}'
    )


def lenient_json(raw: str):
    """JSON estrito → strict=False → reparo de truncamento → None."""
    import re
    s = strip_fence(raw)
    cands = [s, re.sub(r",\s*([\]}])", r"\1", s)]
    for candidate in cands:
        for txt in (candidate, candidate + '"', candidate + '"}]', candidate + "]}", candidate + "}"):
            try:
                return json.loads(txt, strict=False)
            except Exception:
                continue
    try:
        obj, _ = json.JSONDecoder().raw_decode(s.strip().lstrip("["), strict=False)
        return obj if isinstance(obj, (dict, list)) else None
    except Exception:
        return None


def salvage_labels(raw: str) -> list:
    """Último recurso: extrai objetos por regex (qos pode vir sozinho)."""
    import re
    out = []
    for m in re.finditer(r'\{[^{}]*?"id"\s*:\s*"[^"]+"[^{}]*?\}', raw, re.S):
        blob = m.group(0)
        rid = re.search(r'"id"\s*:\s*"([^"]+)"', blob)
        qos = re.search(r'"qos_profile"\s*:\s*"(QI_[A-Z_]+)"', blob)
        if rid and qos:
            out.append({"id": rid.group(1), "qos_profile": qos.group(1), "_partial": True})
    return out


def parse_label(raw: str) -> dict | None:
    try:
        d = json.loads(strip_fence(raw))
        if d.get("qos_profile") in QOS_CRITERIA and isinstance(d.get("caps"), dict):
            d["complexity"] = max(0, min(3, int(d.get("complexity", 1))))
            d["urgency"] = max(0, min(2, int(d.get("urgency", 1))))
            d["needs_planning"] = bool(d.get("needs_planning"))
            d["caps"] = {c: bool(d["caps"].get(c, False)) for c in CAPS}
            d["confidence"] = float(d.get("confidence", 0.5))
            return d
    except Exception:
        return None
    return None


def label_all(src: str, out: str, style: int, temp: float, chunk: int = 5) -> None:
    seen = done_ids(out)
    rows = []
    with open(src) as f:
        for line in f:
            row = json.loads(line)
            if "text" in row and f"{row['id']}:s{style}" not in seen:
                rows.append(row)
    for i in range(0, len(rows), chunk):
        part = rows[i:i + chunk]
        texts = "\n\n".join(f"[{r['id']}] \"\"\"{r['text']}\"\"\"" for r in part)
        prompt = (
            f"{_label_prompt(style, '')}\n\n"
            f"Label EACH numbered request below INDEPENDENTLY (no influence between them).\n"
            f'IMPORTANT: write "evidence" as a SHORT PARAPHRASE in your own words (max 12 words, NO quotation marks inside strings).\n'
        f'Return ONLY a JSON array with one object per request, each with its "id": '
            f'[{{"id": "...", "qos_profile": "...", "needs_planning": bool, "complexity": 0-3, '
            f'"urgency": 0-2, "caps": {{...8 bools...}}, "evidence": "<=200 chars", "confidence": 0-1}}, ...]\n\n'
            f"{texts}"
        )
        raw = chat(prompt, temp, maxtok=3000, no_think=True)
        arr = lenient_json(raw)
        items = arr if isinstance(arr, list) else []
        if not items:
            salv = salvage_labels(raw)
            if salv:
                for it in salv:
                    append(out, {"id": f"{it['id']}:s{style}",
                                 "label": {"qos_profile": it["qos_profile"], "_partial": True}})
            else:
                append(out, {"id": f"chunk{i}:s{style}", "error": raw[-160:]})
            continue
        for it in items:
            if not isinstance(it, dict) or "id" not in it:
                continue
            lab = dict(it.get("label", it))
            lab.pop("id", None)
            lab = parse_label(json.dumps(lab))
            if lab:
                append(out, {"id": f"{it['id']}:s{style}", "label": lab})


def main() -> None:
    ap = argparse.ArgumentParser()
    ap.add_argument("--target-gen", type=int, default=1400, help="batches de 3 textos")
    ap.add_argument("--out", default="/tmp/laya_t904_gen")
    ap.add_argument("--seed", type=int, default=904)
    ap.add_argument("--label-b-frac", type=float, default=0.25,
                    help="fração ESTRATIFICADA (delivery×lang) que recebe a 2ª anotação independente (κ)")
    ap.add_argument("--phase", choices=["gen", "label", "all"], default="all")
    args = ap.parse_args()
    os.makedirs(args.out, exist_ok=True)
    gen = f"{args.out}/gen.jsonl"
    t0 = time.time()
    if args.phase in ("gen", "all"):
        gen_texts(args.target_gen, gen, args.seed)
        n = sum(1 for _ in open(gen))
        print(f"[GEN done] {n} rows in {time.time()-t0:.0f}s", flush=True)
    if args.phase == "gen":
        print("[gen-only] labels não executadas nesta invocação", flush=True)
        return
    label_all(gen, f"{args.out}/label_a.jsonl", style=0, temp=0.0)
    print(f"[LABEL_A done] {time.time()-t0:.0f}s", flush=True)
    if args.label_b_frac > 0:
        rng = random.Random(args.seed + 7)
        by_stratum: dict = {}
        with open(gen) as f:
            for line in f:
                row = json.loads(line)
                if "text" not in row:
                    continue
                by_stratum.setdefault((row.get("delivery"), row.get("lang")), []).append(row["id"])
        subset: set = set()
        for key, ids in sorted(by_stratum.items()):
            k = max(1, round(len(ids) * args.label_b_frac))
            subset.update(rng.sample(sorted(ids), k))
        tmp = f"{args.out}/gen_subset_b.jsonl"
        if not os.path.exists(tmp):
            with open(gen) as f, open(tmp, "w") as g:
                for line in f:
                    row = json.loads(line)
                    if row.get("id") in subset:
                        g.write(line)
        label_all(tmp, f"{args.out}/label_b.jsonl", style=1, temp=0.2)
    print(f"[ALL done] total {time.time()-t0:.0f}s", flush=True)


if __name__ == "__main__":
    main()
