#!/usr/bin/env python3
"""T-900-A2 — script AUTOCONTIDO para `colab run` (VM fresca → gera → solta).

Faz TUDO: baixa modelo (verifica pin), baixa llama-server CUDA, sobe o
server, gera N exemplos, imprime o JSONL em stdout entre marcadores
(CHUNK_BEGIN/CHUNK_END) para o CLI capturar localmente.

Uso local: colab run --gpu L4 chunk_main.py -- --n 3000 --seed 101
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

EXPECTED_SHA = "90fd944d227e9d9b68e7e2c7d5b57b79d4c66ed521b0919fbbd932cf834f6f8e"
GGUF_URL = ("https://huggingface.co/unsloth/gemma-4-12b-it-GGUF/resolve/"
            "main/gemma-4-12b-it-UD-Q4_K_XL.gguf")
LLAMA_URL = ("https://github.com/ggml-org/llama.cpp/releases/download/b11671/"
             "llama-b11671-bin-ubuntu-cuda-12.8-x64.tar.gz")
CUDART_URL = ("https://github.com/ggml-org/llama.cpp/releases/download/b11671/"
              "cudart-llama-b11671-bin-ubuntu-cuda-12.8-x64.tar.gz")

CAPS = ["rust_refactor", "python_refactor", "unit_testing", "security_audit",
        "code_review", "research", "summarization", "translation"]
QOS_PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]
QOS_DESC = {
    "QI_BALANCED": "um trabalho comum",
    "QI_CRITICAL": "um trabalho crítico e urgente",
    "QI_STREAM_LIKE": "um trabalho interativo",
    "QI_LOW_COST": "um trabalho em lote barato",
    "QI_FAILOVER": "um trabalho que exige resiliência a falhas",
}
CAP_PT = {
    "rust_refactor": "refatoração de código Rust", "python_refactor": "refatoração de código Python",
    "unit_testing": "escrita de testes", "security_audit": "auditoria de segurança",
    "code_review": "revisão de código", "research": "pesquisa", "summarization": "resumo",
    "translation": "tradução",
}


def setup(base: Path) -> Path:
    gguf = base / "models/gemma-4-12b-it-UD-Q4_K_XL.gguf"
    if not gguf.exists():
        gguf.parent.mkdir(parents=True, exist_ok=True)
        print("[chunk] baixando modelo (7,4 GB)…", flush=True)
        subprocess.run(["curl", "-sL", "--retry", "3", "-o", str(gguf) + ".part", GGUF_URL], check=True)
        os.rename(str(gguf) + ".part", gguf)
    got = hashlib.sha256(gguf.read_bytes()).hexdigest()
    if got != EXPECTED_SHA:
        print(f"FATAL pin: {got}", file=sys.stderr)
        sys.exit(2)
    print("[chunk] pin do modelo OK", flush=True)

    llm = base / "llm"
    if not (llm / "llama-b11671/llama-server").exists():
        llm.mkdir(parents=True, exist_ok=True)
        for url, name in [(LLAMA_URL, "llama.tar.gz"), (CUDART_URL, "cudart.tar.gz")]:
            subprocess.run(["curl", "-sL", "--retry", "3", "-o", str(llm / name), url], check=True)
        subprocess.run(["tar", "xzf", "llama.tar.gz"], cwd=llm, check=True)
        subprocess.run(["bash", "-c", "mkdir -p cudart && tar xzf cudart.tar.gz -C cudart"], cwd=llm, check=True)
    print("[chunk] llama-server pronto", flush=True)

    env = dict(os.environ)
    env["LD_LIBRARY_PATH"] = (
        f"{llm/'llama-b11671'}:{llm/'cudart/cudart-llama-b11671-bin-ubuntu-cuda-12.8-x64'}:"
        "/usr/lib64-nvidia")
    srv = subprocess.Popen(
        [str(llm / "llama-b11671/llama-server"), "-m", str(gguf),
         "-ngl", "99", "-np", "1", "-c", "8192", "--host", "127.0.0.1", "--port", "8901"],
        env=env, stdout=open("/tmp/llama.log", "w"), stderr=subprocess.STDOUT,
        start_new_session=True)
    for i in range(120):
        time.sleep(5)
        try:
            with urllib.request.urlopen("http://127.0.0.1:8901/health", timeout=2) as r:
                if b"ok" in r.read():
                    print(f"[chunk] server HEALTH OK ({(i+1)*5}s)", flush=True)
                    return srv
        except Exception:
            pass
    print("FATAL: server sem health", file=sys.stderr)
    sys.exit(3)


# ---- geração (espelho do gen_dataset.py, sem imports locais) -----------

def sample_spec(rng):
    lang = rng.choice(["pt", "en"])
    n_caps = rng.choices([1, 2, 3], weights=[50, 35, 15])[0]
    caps = rng.sample(CAPS, n_caps)
    planning = rng.random() < 0.35
    urgency = rng.choices(["low", "normal", "high"], weights=[30, 50, 20])[0]
    qos = rng.choices(QOS_PROFILES, weights=[40, 15, 10, 20, 15])[0]
    complexity = min(3, n_caps + (1 if planning else 0))
    return {"lang": lang, "caps": sorted(caps), "planning": planning,
            "urgency": urgency, "qos": qos, "complexity": complexity}


def gen_prompt(spec):
    caps = spec["caps"]
    if spec["lang"] == "pt":
        cap_txt = ", ".join(CAP_PT[c] for c in caps)
        plan = ("O pedido deve envolver múltiplas etapas dependentes."
                if spec["planning"] else "O pedido deve ser de etapa ÚNICA.")
        urg = {"low": "sem urgência", "normal": "urgência normal", "high": "urgente"}[spec["urgency"]]
        return (f"Escreva UM pedido curto (1-3 frases) de usuário, em português, "
                f"que exija: {cap_txt}. Contexto: {spec['qos']}, {urg}. {plan} "
                f"Responda SOMENTE com o pedido.")
    cap_txt = ", ".join(c.replace("_", " ") for c in caps)
    plan = ("The request must involve multiple dependent steps."
            if spec["planning"] else "The request must be SINGLE-step.")
    urg = {"low": "no urgency", "normal": "normal urgency", "high": "urgent"}[spec["urgency"]]
    return (f"Write ONE short user request (1-3 sentences), in English, "
            f"requiring: {cap_txt}. Context: {spec['qos']}, {urg}. {plan} "
            f"Reply ONLY with the request.")


def call_llm(prompt, temperature):
    body = json.dumps({"messages": [{"role": "user", "content": prompt}],
                       "max_tokens": 1024, "temperature": temperature}).encode()
    last = None
    for attempt in range(5):
        try:
            req = urllib.request.Request("http://127.0.0.1:8901/v1/chat/completions",
                                         data=body, headers={"Content-Type": "application/json"})
            with urllib.request.urlopen(req, timeout=180) as resp:
                data = json.load(resp)
            msg = data["choices"][0]["message"]
            return extract_answer((msg.get("content") or msg.get("reasoning_content") or "").strip())
        except Exception as e:
            last = e
            time.sleep(2 * (attempt + 1))
    raise last


def extract_answer(raw):
    lines = [l.strip().strip("*").strip() for l in raw.splitlines()]
    cands = [l for l in lines if l and not l.startswith(("-", "*", "•", "#"))
             and ":" not in l[:24] and len(l) >= 15]
    if not cands:
        quoted = [l for l in lines if '"' in l and len(l) >= 15]
        if quoted:
            return quoted[-1].strip('"').strip()
        return ""
    return cands[-1]


def lang_ok(text, lang):
    pt = sum(text.lower().count(w) for w in [" o ", " a ", " de ", " que ", " para ", " e "])
    en = sum(text.lower().count(w) for w in [" the ", " and ", " to ", " for ", " with ", " of "])
    return pt >= en if lang == "pt" else (en > pt or (en == 0 and pt == 0))


def questions_all():
    q = {
        "qos_profile": {"type": "choice",
                         "instructions": "Which QoS objective best fits this workload?",
                         "criteria": {p: QOS_DESC[p] for p in QOS_PROFILES}},
        "needs_planning": {"type": "noul",
                            "instructions": "Does the request require multiple dependent tasks?"},
        "complexity": {"type": "score", "instructions": "How complex is the task?",
                        "criteria": ["simple", "moderate", "complex", "very_complex"]},
        "urgency": {"type": "score", "instructions": "How urgent is the task?",
                     "criteria": ["low", "normal", "high"]},
    }
    for cap in CAPS:
        q[f"cap__{cap}"] = {"type": "noul",
                              "instructions": f"Does this request require the capability '{cap}'?"}
    return q


def main():
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, default=3000)
    ap.add_argument("--seed", type=int, default=101)
    args = ap.parse_args()

    base = Path("/content/colab_gen")
    base.mkdir(parents=True, exist_ok=True)
    srv = setup(base)

    import random
    rng = random.Random(args.seed)
    qs = questions_all()
    seen = set()
    rows = []
    attempts = 0
    print(f"PROG 0 {args.n}", flush=True)
    while len(rows) < args.n and attempts < args.n * 6:
        attempts += 1
        spec = sample_spec(rng)
        try:
            text = call_llm(gen_prompt(spec), 0.9)
        except Exception:
            continue
        text = re.sub(r"^(pedido|request)\s*:?\s*", "", text, flags=re.I).strip().strip('"')
        if not (20 <= len(text) <= 600):
            continue
        if not lang_ok(text, spec["lang"]):
            continue
        key = re.sub(r"\s+", " ", text.lower()).strip()[:120]
        if key in seen:
            continue
        seen.add(key)
        expected = {"qos_profile": spec["qos"], "needs_planning": bool(spec["planning"]),
                     "complexity": spec["complexity"],
                     "urgency": ["low", "normal", "high"].index(spec["urgency"]),
                     **{f"cap__{c}": (c in spec["caps"]) for c in CAPS}}
        rows.append({"state": {"text": text}, "questions": qs, "expected": expected,
                      "language": spec["lang"],
                      "tags": [f"lang:{spec['lang']}", f"qos:{spec['qos']}",
                                f"planning:{int(spec['planning'])}",
                                f"caps:{'+'.join(spec['caps'])}",
                                "provenance:synthetic-by-construction-gemma4"]})
        if len(rows) % 100 == 0:
            print(f"PROG {len(rows)} {args.n}", flush=True)

    print(f"PROG {len(rows)} DONE", flush=True)
    print("CHUNK_BEGIN", flush=True)
    for r in rows:
        print(json.dumps(r, ensure_ascii=False), flush=True)
    print("CHUNK_END", flush=True)
    srv.terminate()


if __name__ == "__main__":
    main()
