#!/usr/bin/env python3
"""T912-F1 — Gerador de DOSES (variantes raras e caps). Derivado do gen_t910_contrast.py.

Diferenças vs T910:
- DOMAINS expandidos (40) e specs reequilibradas para as classes FAMINTAS da T911:
  STREAM/LOW_COST/FAILOVER (qos), cx3, urg1/urg2, e TODAS as 8 caps com slots extras
  para as 5 mais raras (rust_refactor, python_refactor, unit_testing, code_review,
  security_audit);
- 4 textos por lote (antes 3);
- host/port/model parametrizáveis (roda na .61 gemma Q4 OU local gemma Q8);
- eixo T912-QOS-RARE só com as 3 raras (+ 1 hard-neg BALANCED);
- seed 912.

Resumível por batch. Saída: gen_{axis}.jsonl no --out-root (PERSISTENTE, nunca /tmp).
"""
import argparse
import http.client
import itertools
import json
import os
import re
import time

DOMAINS = [
    # 20 originais do T910 (mantidos p/ continuidade de distribuição)
    "controle de acesso hospitalar", "dashboard de qualidade de dados", "sincronização multi-região",
    "editor colaborativo em tempo real", "auditoria de logs de banco", "robô de atendimento telefônico",
    "relatório de pegada de carbono", "validador de regulamentação LGPD", "chatbot de horóscopo",
    "sistema de aposta esportiva", "pipeline de renderização de vídeo", "catálogo de vinícola",
    "gestão de frota de drones", "protocolo de experimento laboratorial", "radar de reclamações",
    "gerenciador de assinaturas", "tradutor de código legado", "verificador de candidatos a crédito",
    "monitor de colmeias", "reserva de quadras esportivas",
    # 20 novos (T912)
    "portal de matrículas escolares", "controle de estoque de farmácia", "fábrica de cerveja artesanal",
    "rastreamento de cargas frigorificadas", "plataforma de cursos online", "gestão de contratos públicos",
    "app de carona solidária", "manutenção predial preditiva", "serviço de telemedicina",
    "bolsa de valores simulada", "gestão de resíduos recicláveis", "redação jornalística automatizada",
    "controle de ponto e jornada", "gestão de obras civis", "monitoramento de gado de corte",
    "sistema de bilhetagem de transporte", "orquestra de sinais de trading", "gestão de eventos e congressos",
    "inspeção de tubulações por drone", "plataforma de doações beneficentes",
]
LANGS = [("pt", "Portuguese (Brazil)"), ("en", "English")]

# Cada spec: (kind, gold, descrição). Slots duplicados = dose maior.
AXES = {
    # 3 raras + 1 hard-neg BALANCED (CRITICAL/BALANCED já abundantes no pool)
    "T912-QOS-RARE": [
        ("qos", "QI_STREAM_LIKE", "user explicitly asks to RECEIVE each usable partial result AS IT IS finished ('show me each translation as you finish it', 'send each verified batch right away'); partial-delivery MUST be explicit in the text"),
        ("qos", "QI_STREAM_LIKE", "interactive back-and-forth deliverable: user wants the first usable piece IMMEDIATELY and the rest incrementally as ready (explicit in text), e.g. live digest of incoming results"),
        ("qos", "QI_LOW_COST", "explicit cost priority: cheapest tier, idle capacity, minimize spend, budget-capped batch job"),
        ("qos", "QI_LOW_COST", "off-peak / spare-capacity processing where the user explicitly says cost matters more than speed or quality polish"),
        ("qos", "QI_FAILOVER", "explicit survival requirement: fallback provider, retries with backoff, degraded-mode continuity if a node or vendor dies"),
        ("qos", "QI_FAILOVER", "business continuity: the text requires the pipeline to KEEP WORKING through provider outage, with a standby route and automatic switchover"),
        ("qos", "QI_BALANCED", "HARD NEGATIVE: internal parallel/streaming processing mentioned, but the user wants ONE consolidated result only at the END"),
    ],
    "S-CX3": [
        ("cx", 3, "multi-phase data-integrity migration with verification gates and a rollback plan; cross-team coordination; explicit uncertainty about legacy behavior"),
        ("cx", 3, "investigation of an intermittent race condition with deep uncertainty, requiring reproduction attempts, root-cause analysis and a safe fix strategy"),
        ("cx", 3, "architectural overhaul of authentication across services with migration of sessions, risk assessment and staged rollout with revert criteria"),
        ("cx", 3, "open-ended performance war-room: recurring latency spikes with no known cause, needs instrumentation, hypothesis testing, cost/benefit fixes and validation under load"),
        ("cx", 2, "SAME STRUCTURE AND LENGTH AS A 3-LEVEL PROJECT, but the work is a straightforward checklist of well-understood steps with no risk, no uncertainty and no architectural decision"),
        ("cx", 1, "HARD NEGATIVE: a long multi-phase plan (6-8 steps listed as a checklist) where EVERY step is trivial and well-understood (rename files, update titles, reformat)"),
        ("cx", 2, "a short 2-3 sentence request about debugging data corruption between two systems with integrity risk"),
    ],
    "S-URG": [
        ("urg", 1, "THE SAME work request with a concrete intermediate deadline ('by Friday', 'before the demo next week', 'in two days')"),
        ("urg", 1, "work tied to a scheduled external event that is NOT today (board meeting Thursday, release train tomorrow end of day, audit visit next Monday)"),
        ("urg", 2, "THE SAME work request but production is DOWN RIGHT NOW / users are blocked NOW / this is a live incident needing a fix in minutes"),
        ("urg", 0, "HARD NEGATIVE: rhetorical speed words with no real consequence ('quick question about...', 'asap if convenient', 'small favor') and no deadline"),
    ],
    # TODAS as 8 caps; slots dobrados p/ as 5 famintas; 2 hard-negs
    "S-CAPS8": [
        ("cap", ["rust_refactor"], "restructure EXISTING Rust code: split a god-module, remove unsafe blocks, introduce traits, migrate to a new API version — the artifact already exists"),
        ("cap", ["rust_refactor"], "Rust modernization/refactor of a live crate (borrow-checker pain, lifetimes, module boundaries) on EXISTING code"),
        ("cap", ["python_refactor"], "restructure EXISTING Python code: break a monolith into services/modules, apply patterns, reduce cyclomatic complexity — existing codebase"),
        ("cap", ["python_refactor"], "Python legacy cleanup of an existing repository (typing migration, de-duplication, layering) — explicitly NOT new features"),
        ("cap", ["unit_testing"], "write automated tests for EXISTING code ('add pytest coverage for the parser module', 'characterization tests before the migration')"),
        ("cap", ["unit_testing"], "build a test suite (unit + regression) around an existing component, including edge cases and fixtures"),
        ("cap", ["security_audit"], "REVIEW existing code/config FOR vulnerabilities/safety/compliance — 'review this service for security vulnerabilities', 'check the auth flow can't be exploited'"),
        ("cap", ["security_audit"], "threat-model / penetration review of an EXISTING system or deployment (secrets handling, authz gaps, injection surfaces)"),
        ("cap", ["code_review"], "quality review of EXISTING changes/code: readability, structure, bugs without security focus ('review the PR for maintainability')"),
        ("cap", ["code_review"], "review an existing merge request for correctness and design coherence; suggest refactorings; NO security angle"),
        ("cap", ["research"], "investigate state of the art / compare options / survey sources on a topic and produce a recommendation"),
        ("cap", ["summarization"], "condense an EXISTING long document/thread/report into an executive summary or digest"),
        ("cap", ["translation"], "translate an EXISTING document/thread with technical fidelity (any language pair)"),
        ("cap", [], "HARD NEGATIVE: 'explain how our testing strategy works' / 'write an essay about unit testing' — discussing, NOT doing"),
        ("cap", [], "HARD NEGATIVE: brand-new small feature implementation (build from scratch, no existing artifact involved)"),
    ],
}


class Seq:
    def __init__(self, seed):
        self.state = seed & 0x7FFFFFFF or 1

    def next(self):
        self.state = (self.state * 48271) % 0x7FFFFFFF
        return self.state

    def shuffle(self, items):
        for i in range(len(items) - 1, 0, -1):
            j = self.next() % (i + 1)
            items[i], items[j] = items[j], items[i]


def chat(host, port, model, prompt, temperature=0.9, maxtok=4000):
    msgs = [{"role": "user", "content": prompt}]
    payload = json.dumps({"model": model, "messages": msgs,
                          "max_tokens": maxtok, "temperature": temperature})
    headers = {"Content-Type": "application/json", "Host": f"{host}:{port}"}
    for retry in range(4):
        try:
            conn = http.client.HTTPConnection(host, port, timeout=900)
            conn.request("POST", "/v1/chat/completions", body=payload, headers=headers)
            out = json.loads(conn.getresponse().read())
            content = out["choices"][0]["message"].get("content") or ""
            conn.close()
            if content.strip():
                return content
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
    s = s.strip()
    for c in (s, re.sub(r",\s*([\]}])", r"\1", s)):
        for t in (c, c + '"', c + "]"):
            try:
                return json.loads(t, strict=False)
            except Exception:
                continue
    return None


def done_batches(path):
    try:
        return {json.loads(l)["batch"] for l in open(path) if l.strip()}
    except Exception:
        return set()


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--out-root", required=True)
    ap.add_argument("--axis", required=True, choices=list(AXES))
    ap.add_argument("--batches", type=int, required=True)
    ap.add_argument("--seed", type=int, default=912)
    ap.add_argument("--host", default="127.0.0.1")
    ap.add_argument("--port", type=int, default=8083)
    ap.add_argument("--model", default="gemma")
    args = ap.parse_args()
    os.makedirs(args.out_root, exist_ok=True)
    genp = os.path.join(args.out_root, f"gen_{args.axis}.jsonl")

    cells = list(itertools.product(AXES[args.axis], DOMAINS, LANGS))
    rng = Seq(args.seed)
    rng.shuffle(cells)

    done = done_batches(genp)
    i = 0
    while len(done) < args.batches and i < len(cells):
        spec, domain, (lang, ldesc) = cells[i]
        i += 1
        bid = f"{args.axis.lower()}-{len(done):04d}"
        if bid in done:
            continue
        kind, gold, desc = spec
        parts = [f"- domain: {domain}", f"- language: {ldesc}", f"- {desc}"]
        if kind == "cap" and gold:
            parts.append(f"- capability required: {', '.join(gold)} (half WITHOUT naming it literally)")
        prompt = (
            "You produce REALISTIC workflow requests for an orchestration system.\n"
            "Internal spec (do NOT mention the spec, do NOT use words 'QoS', 'stage count', 'capability'):\n"
            + "\n".join(parts) +
            "\nWrite 4 DIFFERENT natural requests (2-4 sentences each). Requirements in the spec "
            "MUST be readable from the request text alone.\n"
            'Return ONLY JSON: [{"text": "..."}, ...]')
        raw = chat(args.host, args.port, args.model, prompt)
        arr = lenient_json(raw) or []
        for j, it in enumerate(arr[:4]):
            if isinstance(it, dict) and it.get("text"):
                rec = {"id": f"{bid}-{j}", "batch": bid, "axis": args.axis,
                       "kind": kind, "gold": gold, "domain": domain, "lang": lang,
                       "text": str(it["text"])[:1200]}
                with open(genp, "a") as f:
                    f.write(json.dumps(rec, ensure_ascii=False) + "\n")
        done = done_batches(genp)
        if len(done) % 20 == 0:
            print(f"  {args.axis}: batches={len(done)}/{args.batches}", flush=True)
    print(f"[{args.axis} done] {len(done_batches(genp))} batches -> {genp}", flush=True)


if __name__ == "__main__":
    main()
