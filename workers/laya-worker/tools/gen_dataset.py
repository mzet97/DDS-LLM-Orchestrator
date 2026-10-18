#!/usr/bin/env python3
"""T-900-A1 — geração do dataset calibração (~2.000, formato SDK laya).

Rótulo POR CONSTRUÇÃO: amostra um spec (idioma/qos/planning/capabilities/
urgência/complexidade) e pede ao gemma local que ESCREVA um pedido de
usuário que exija exatamente esse spec. O rótulo é verdade por construção
(com ruído do gerador — documentado; validação amostral no relatório).

Formato: JSONL nativo do SDK (evals.Dataset.from_jsonl):
{"state":{"text":...}, "questions": {...QUESTIONS_V1 adaptadas...},
 "expected":{...}, "language":"pt|en", "tags":{"spec":...}}

Uso: gen_dataset.py <out.jsonl> [--n 2000] [--resume]
Requer llama-server em 127.0.0.1:8901 (o mesmo da campanha).
"""

import argparse
import json
import random
import re
import sys
import time
import urllib.request
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))
from laya_worker.questions import CAPABILITY_CATALOG, QOS_PROFILES  # noqa: E402

SERVER = "http://127.0.0.1:8901"

CAP_PT = {
    "rust_refactor": "refatoração de código Rust",
    "python_refactor": "refatoração de código Python",
    "unit_testing": "escrita de testes de unidade",
    "security_audit": "auditoria de segurança",
    "code_review": "revisão de código",
    "research": "pesquisa/levantamento",
    "summarization": "resumo/síntese",
    "translation": "tradução",
}
QOS_DESC = {
    "QI_BALANCED": "um trabalho comum, sem restrições especiais",
    "QI_CRITICAL": "um trabalho crítico, urgente e sensível a prazo (produção)",
    "QI_STREAM_LIKE": "um trabalho interativo com resposta em streaming",
    "QI_LOW_COST": "um trabalho em lote, barato, sem urgência",
    "QI_FAILOVER": "um trabalho que exige resiliência a falhas de infraestrutura",
}


def sample_spec(rng: random.Random) -> dict:
    lang = rng.choice(["pt", "en"])
    n_caps = rng.choices([1, 2, 3], weights=[50, 35, 15])[0]
    caps = rng.sample(CAPABILITY_CATALOG, n_caps)
    needs_planning = rng.random() < 0.35
    urgency = rng.choices(["low", "normal", "high"], weights=[30, 50, 20])[0]
    qos = rng.choices(QOS_PROFILES, weights=[40, 15, 10, 20, 15])[0]
    complexity = min(3, n_caps + (1 if needs_planning else 0))
    return {"lang": lang, "caps": sorted(caps), "planning": needs_planning,
            "urgency": urgency, "qos": qos, "complexity": complexity}


def gen_prompt(spec: dict) -> str:
    lang = spec["lang"]
    caps = spec["caps"]
    if lang == "pt":
        cap_txt = ", ".join(CAP_PT[c] for c in caps)
        conj = " e em seguida " if spec["planning"] else ". "
        plan = (f"O pedido deve envolver múltiplas etapas dependentes (ex.: etapa A{conj}etapa B)."
                if spec["planning"] else "O pedido deve ser de etapa ÚNICA, sem sub-etapas dependentes.")
        urg = {"low": "sem urgência", "normal": "urgência normal", "high": "urgente"}[spec["urgency"]]
        qos = QOS_DESC[spec["qos"]].replace("um trabalho", "do trabalho")
        return (
            f"Escreva UM pedido curto (1-3 frases) de usuário para um sistema de agentes, "
            f"em português, que exija: {cap_txt}. Contexto: {qos}, {urg}. {plan} "
            f"Responda SOMENTE com o pedido, sem títulos ou explicações."
        )
    cap_txt = ", ".join(c.replace("_", " ") for c in caps)
    plan = ("The request must involve multiple dependent steps (e.g., step A then step B)."
            if spec["planning"] else "The request must be SINGLE-step, no dependent sub-steps.")
    urg = {"low": "no urgency", "normal": "normal urgency", "high": "urgent"}[spec["urgency"]]
    qos = QOS_DESC[spec["qos"]].replace("a ", "a ")
    return (
        f"Write ONE short user request (1-3 sentences) for an agent system, in English, "
        f"requiring: {cap_txt}. Context: {qos}, {urg}. {plan} "
        f"Reply ONLY with the request, no titles or explanations."
    )


def questions_for(caps_planning_only: bool = False) -> dict:
    q = {
        "qos_profile": {
            "type": "choice",
            "instructions": "Which QoS objective best fits this workload?",
            "criteria": {p: QOS_DESC[p] for p in QOS_PROFILES},
        },
        "needs_planning": {"type": "noul", "instructions": "Does the request require multiple dependent tasks?"},
        "complexity": {"type": "score", "instructions": "How complex is the task?",
                        "criteria": ["simple", "moderate", "complex", "very_complex"]},
        "urgency": {"type": "score", "instructions": "How urgent is the task?",
                     "criteria": ["low", "normal", "high"]},
    }
    for cap in CAPABILITY_CATALOG:
        q[f"cap__{cap}"] = {
            "type": "noul",
            "instructions": f"Does this request require the capability '{cap}'?",
        }
    return q


def call_llm(prompt: str, temperature: float) -> str:
    body = json.dumps({
        "messages": [{"role": "user", "content": prompt}],
        "max_tokens": 1024,
        "temperature": temperature,
    }).encode()
    req = urllib.request.Request(f"{SERVER}/v1/chat/completions", data=body,
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req, timeout=120) as resp:
        data = json.load(resp)
    msg = data["choices"][0]["message"]
    raw = (msg.get("content") or msg.get("reasoning_content") or "").strip()
    return extract_answer(raw)


def extract_answer(raw: str) -> str:
    """O gemma raciocina em bullets antes de responder: fica com a última
    linha não-bullet/não-cabeçalho (a resposta em si vem por último)."""
    lines = [l.strip().strip('*').strip() for l in raw.splitlines()]
    cands = [
        l for l in lines
        if l and not l.startswith(("-", "*", "•", "#")) and ":" not in l[:24]
        and len(l) >= 15
    ]
    if not cands:
        # fallback: última linha com aspas (resposta citada)
        quoted = [l for l in lines if '"' in l and len(l) >= 15]
        if quoted:
            return quoted[-1].strip('"').strip()
        return ""
    return cands[-1]


def lang_ok(text: str, lang: str) -> bool:
    pt_marks = sum(text.lower().count(w) for w in [" o ", " a ", " de ", " que ", " para ", " e "])
    en_marks = sum(text.lower().count(w) for w in [" the ", " and ", " to ", " for ", " with ", " of "])
    if lang == "pt":
        return pt_marks >= en_marks
    return en_marks > pt_marks or (en_marks == 0 and pt_marks == 0)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("out")
    ap.add_argument("--n", type=int, default=2000)
    ap.add_argument("--seed", type=int, default=900)
    ap.add_argument("--resume", action="store_true")
    args = ap.parse_args()

    rng = random.Random(args.seed)
    out = Path(args.out)
    seen_norm = set()
    start = 0
    if args.resume and out.exists():
        with open(out) as f:
            for line in f:
                if line.strip():
                    seen_norm.add(norm(json.loads(line)["state"]["text"]))
                    start += 1
    print(f"gerando {args.n} exemplos (já existem {start}; seed {args.seed})", flush=True)

    with open(out, "a" if args.resume else "w") as f:
        i = start
        attempts = 0
        while i < args.n and attempts < args.n * 6:
            attempts += 1
            spec = sample_spec(rng)
            text = call_llm(gen_prompt(spec), temperature=0.9)
            text = re.sub(r"^(pedido|request)\s*:?\s*", "", text, flags=re.I).strip().strip('"')
            if not (20 <= len(text) <= 600):
                continue
            if not lang_ok(text, spec["lang"]):
                continue
            key = norm(text)
            if key in seen_norm:
                continue
            seen_norm.add(key)
            expected = {
                "qos_profile": spec["qos"],
                "needs_planning": bool(spec["planning"]),
                "complexity": spec["complexity"],
                "urgency": ["low", "normal", "high"].index(spec["urgency"]),
                **{f"cap__{c}": (c in spec["caps"]) for c in CAPABILITY_CATALOG},
            }
            row = {
                "state": {"text": text},
                "questions": questions_for(),
                "expected": expected,
                "language": spec["lang"],
                "tags": [f"lang:{spec['lang']}", f"qos:{spec['qos']}",
                     f"planning:{int(spec['planning'])}", f"caps:{'+'.join(spec['caps'])}",
                     "provenance:synthetic-by-construction-gemma4"],
            }
            f.write(json.dumps(row, ensure_ascii=False) + "\n")
            i += 1
            if i % 50 == 0:
                f.flush()
                print(f"  {i}/{args.n}", flush=True)
    print(f"pronto: {i} exemplos em {out}", flush=True)
    return 0


def norm(t: str) -> str:
    return re.sub(r"\s+", " ", t.lower()).strip()[:120]


if __name__ == "__main__":
    sys.exit(main())
