#!/usr/bin/env python3
"""O-1: mede TTFC/ICL de N submits DDS streaming + paridade vs HttpEngine.

Uso: o1_measure.py <n> [--mode stream|nostream] [--jsonl out.jsonl]
Requer: servidor :8091 (alias gemma-4-12b-it-UD-Q4_K_XL.gguf) + agente no
domínio 79 com o engine sob teste já rodando.
"""
import argparse
import json
import statistics
import subprocess
import sys
import time

BIN = "/var/tmp/tese-target-bbench/release/submit-one"
MODEL = "gemma-4-12b-it-UD-Q4_K_XL.gguf"
ENV_URI = "file:///var/tmp/o1_plain.xml"

PROMPTS = [
    "Explique em 2 frases o que é o protocolo DDS.",
    "Liste 3 vantagens de pub/sub sobre requisição-resposta.",
    "O que é QoS em sistemas distribuídos? Responda em 1 frase.",
    "Escreva uma frase sobre tolerância a falhas.",
    "Defina latência em sistemas de mensageria em 1 frase.",
    "Qual a diferença entre TCP e UDP? Responda brevemente.",
    "Escreva 2 frases sobre serialização de dados.",
    "O que é um tópico em publish/subscribe? 1 frase.",
    "Explique o que é 'best effort' vs 'reliable' em 2 frases.",
    "Dê um exemplo de uso de filas de mensagens em 1 frase.",
    "Escreva 2 frases sobre escalabilidade horizontal.",
    "O que é um broker de mensagens? 1 frase.",
    "Explique 'at-least-once' em 2 frases.",
    "Escreva 1 frase sobre ordenação de mensagens.",
    "O que é multicast? 1 frase.",
    "Explique o que é um participant em DDS em 1 frase.",
    "Escreva 2 frases sobre observabilidade distribuída.",
    "Defina throughput em mensageria em 1 frase.",
    "O que é backpressure? 1 frase.",
    "Escreva 1 frase sobre idempotência.",
]


def run_one(prompt: str, stream: bool, timeout_ms: int = 150_000) -> dict:
    cmd = [BIN, "79", MODEL, json.dumps([{"role": "user", "content": prompt}]),
           "--timeout-ms", str(timeout_ms), "--max-tokens", "2048",
           "--temperature", "0"]
    if stream:
        cmd.append("--stream")
    t0 = time.time()
    p = subprocess.run(cmd, capture_output=True, text=True, timeout=timeout_ms / 1000 + 30,
                       env={"CYCLONEDDS_URI": ENV_URI, "PATH": "/usr/bin:/bin"})
    wall = time.time() - t0
    line = p.stdout.strip().splitlines()[-1] if p.stdout.strip() else ""
    if not line.startswith("{"):
        return {"ok": False, "error": (p.stderr or line)[:200]}
    d = json.loads(line)
    d["ok"] = bool(d.get("success"))
    d["wall_s"] = round(wall, 2)
    d["prompt"] = prompt
    return d


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("n", type=int)
    ap.add_argument("--mode", choices=["stream", "nostream"], default="stream")
    ap.add_argument("--jsonl")
    args = ap.parse_args()

    results = []
    for i in range(args.n):
        r = run_one(PROMPTS[i % len(PROMPTS)], args.mode == "stream")
        results.append(r)
        status = (f"ttfc={r['ttfc_ms']}ms total={r['latency_ms']}ms "
                  f"({100 * r['ttfc_ms'] / max(r['latency_ms'], 1):.0f}%) tok={r.get('tokens_completion')}"
                  if r.get("ok") else f"FALHOU: {r.get('error', '?')[:80]}")
        print(f"[{i + 1}/{args.n}] {status}", flush=True)

    ok = [r for r in results if r.get("ok")]
    if not ok:
        print("TODOS FALHARAM")
        return 1
    ttfc = [r["ttfc_ms"] for r in ok]
    total = [r["latency_ms"] for r in ok]
    ratio = [100 * r["ttfc_ms"] / max(r["latency_ms"], 1) for r in ok]
    summary = {
        "mode": args.mode, "n": args.n, "ok": len(ok),
        "ttfc_p50_ms": statistics.median(ttfc),
        "ttfc_mean_ms": round(statistics.mean(ttfc), 1),
        "total_p50_ms": statistics.median(total),
        "total_mean_ms": round(statistics.mean(total), 1),
        "ttfc_pct_p50": round(statistics.median(ratio), 1),
        "tokens_completion_mean": round(statistics.mean([r.get("tokens_completion", 0) for r in ok]), 1),
    }
    print("RESUMO " + json.dumps(summary))
    if args.jsonl:
        with open(args.jsonl, "w") as f:
            for r in results:
                f.write(json.dumps(r, ensure_ascii=False) + "\n")
        with open(args.jsonl.replace(".jsonl", "_summary.json"), "w") as f:
            json.dump(summary, f, indent=1)
    return 0


if __name__ == "__main__":
    sys.exit(main())
