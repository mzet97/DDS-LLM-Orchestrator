#!/usr/bin/env python3
"""T912-F1 — Rotulador LLM com política v6 (idêntica à LABEL_POLICY_T909, byte a byte).

Diferenças vs label_t909_corpus.py: --host/--port/--model parametrizáveis (roda na
.61 gemma Q4 OU local gemma Q8). Receita provada: prefill '[', temp 0, chunks de 5.
Resumível por id.
"""
import argparse
import http.client
import json
import re
import time

POLICY_V6 = """Labeling policy v6 (read the text ALONE; judge difficulty, not length):
- qos_profile:
  * QI_CRITICAL ONLY if the text states a concrete failure consequence (production outage, SLA breach, revenue loss, regulatory fine, safety). Emergency words without consequence = QI_BALANCED.
  * QI_STREAM_LIKE ONLY if the user explicitly asks to RECEIVE usable partial results as each piece is finished. Progress bars, parallelism, internal streaming, or one consolidated result at the end = NOT stream.
  * QI_LOW_COST: explicit priority on cost/cheap/idle capacity. QI_FAILOVER: explicit survival of outages (retries/fallback/degraded mode). QI_BALANCED: routine one-shot deliverable.
- needs_planning: true ONLY for DEPENDENT subtasks (output of one feeds the next, or ordered artifacts). Enumerations of independent items of the same kind = false. One single long task = false.
- complexity = DIFFICULTY, not step count:
  * 0: one trivial atomic operation. 1: bounded work with obvious steps, even if MANY (5-8 trivial checklist steps = 1).
  * 2: real design decisions, cross-system dependencies, race conditions, data-integrity risk, deep debugging, uncertainty. 3: project-level multi-phase with validation gates and rollback planning.
  * A short 2-sentence request can be complexity 2 or 3; a long checklist can be 1.
- urgency = TEMPORAL evidence only:
  * 0: no real deadline; rhetorical speed words ('quick question', 'asap if convenient') with no consequence = 0.
  * 1: concrete intermediate deadline for THIS request (by Friday, in two days, before the demo). 2: production down / users blocked / live incident; minutes matter.
- caps (8 booleans): the request REQUIRES the capability.
  * security_audit: reviewing existing code/config for vulnerabilities/safety/compliance — INCLUDING 'review ... for vulnerabilities' phrasing. code_review: quality/bugs review WITHOUT security focus.
  * rust_refactor/python_refactor: restructure EXISTING code (new code is not refactor). unit_testing: tests for existing code. research: investigate/compare/survey. summarization: condense existing material. translation: translate existing material."""


def chat(host, port, model, prompt, maxtok=3000):
    msgs = [{"role": "user", "content": prompt}, {"role": "assistant", "content": "["}]
    payload = json.dumps({"model": model, "messages": msgs,
                          "max_tokens": maxtok, "temperature": 0})
    headers = {"Content-Type": "application/json", "Host": f"{host}:{port}"}
    for retry in range(4):
        try:
            conn = http.client.HTTPConnection(host, port, timeout=900)
            conn.request("POST", "/v1/chat/completions", body=payload, headers=headers)
            out = json.loads(conn.getresponse().read())
            content = out["choices"][0]["message"].get("content") or ""
            conn.close()
            if content.strip():
                return content if content.lstrip().startswith("[") else "[" + content
        except Exception:
            pass
        time.sleep(3 * (retry + 1))
    return ""


def lenient_json(raw):
    s = raw.strip()
    if s.startswith("```"):
        s = s.split("```")[1]
        if s.startswith("json"):
            s = s[4:]
    for c in (s.strip(), re.sub(r",\s*([\]}])", r"\1", s.strip())):
        for t in (c, c + '"', c + "]"):
            try:
                return json.loads(t, strict=False)
            except Exception:
                continue
    try:
        obj, _ = json.JSONDecoder().raw_decode(s.lstrip("["), strict=False)
        return [obj] if isinstance(obj, dict) else obj
    except Exception:
        return None


def done_ids(path):
    try:
        return {json.loads(l)["id"] for l in open(path)}
    except Exception:
        return set()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--gen", required=True)
    ap.add_argument("--out", required=True)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=8083)
    ap.add_argument("--model", default="gemma")
    a = ap.parse_args()
    rows = []
    for line in open(a.gen):
        line = line.strip()
        if line:
            try:
                r = json.loads(line)
                if r.get("text"):
                    rows.append(r)
            except Exception:
                pass
    done = done_ids(a.out)
    rows = [r for r in rows if r["id"] not in done]

    tpl = (POLICY_V6 + "\n\nLabel EACH numbered request INDEPENDENTLY.\n"
           'Return ONLY JSON: [{"id": "...", "qos_profile": "QI_BALANCED|QI_CRITICAL|'
           'QI_STREAM_LIKE|QI_LOW_COST|QI_FAILOVER", "needs_planning": bool, "complexity": 0-3, '
           '"urgency": 0-2, "caps": {"rust_refactor": bool, "python_refactor": bool, '
           '"unit_testing": bool, "security_audit": bool, "code_review": bool, '
           '"research": bool, "summarization": bool, "translation": bool}}, ...]\n\n')
    chunk = 5
    labeled = 0
    for i in range(0, len(rows), chunk):
        part = rows[i:i + chunk]
        texts = "\n\n".join(f'[{r["id"]}] "{r["text"][:600]}"' for r in part)
        arr = lenient_json(chat(a.host, a.port, a.model, tpl + texts)) or []
        for it in arr:
            if not (isinstance(it, dict) and "id" in it and it.get("qos_profile") in
                    ("QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER")):
                continue
            caps = it.get("caps") or {}
            rec = {"id": it["id"], "llm": {
                "qos_profile": it["qos_profile"],
                "needs_planning": bool(it.get("needs_planning")),
                "complexity": max(0, min(3, int(it.get("complexity", 1)))),
                "urgency": max(0, min(2, int(it.get("urgency", 1)))),
                "caps": {c: bool(caps.get(c, False)) for c in
                         ("rust_refactor", "python_refactor", "unit_testing", "security_audit",
                          "code_review", "research", "summarization", "translation")}}}
            with open(a.out, "a") as f:
                f.write(json.dumps(rec, ensure_ascii=False) + "\n")
            labeled += 1
        if (i // chunk) % 20 == 0:
            print(f"  {i}/{len(rows)} -> {labeled}", flush=True)
    print(f"DONE: {labeled}/{len(rows)}", flush=True)


if __name__ == "__main__":
    main()
