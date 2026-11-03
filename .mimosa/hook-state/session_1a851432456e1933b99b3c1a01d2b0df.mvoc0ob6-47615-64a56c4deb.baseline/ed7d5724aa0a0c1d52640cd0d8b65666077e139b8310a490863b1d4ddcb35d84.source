#!/usr/bin/env python3
"""O-4b A/B: DONE do sobrevivente publicado DENTRO da janela de lease do
writer morto deve chegar ao cliente (com o fix) em vez de dar timeout.

Cenário EXP4-style: submit → agente A clama/inicia inferência → kill -9 A
em ~1.2 s → agente B (só entra pós-kill) → reaper re-emfileira com stale 3 s
→ B re-executa → DONE (dentro do lease de ~10 s de A se B for mais fraco
naquele slot). Cliente timeout 30 s.

Uso: o4b_scenario.py <n_runs> [--no-fix]  (env AGENT_NO_ENSURE_TERMINAL=1
nos agentes equivale a --no-fix)
"""
import json
import os
import subprocess
import sys
import time
from pathlib import Path

REPO = Path("/var/mnt/HD1TB/tese/src/rust")
URI = "file:///var/tmp/o1_plain.xml"
SUBMIT = "/var/tmp/tese-target-bbench/release/submit-one"
MODEL = "gemma-4-12b-it-UD-Q4_K_XL.gguf"
DOMAIN = "78"
PROMPT = ("Explique detalhadamente em 4 frases o papel do middleware em "
          "sistemas distribuídos, citando exemplos de problemas que ele resolve.")


def env(extra=None):
    e = {"CYCLONEDDS_URI": URI, "PATH": "/usr/bin:/bin"}
    e.update(extra or {})
    return e


def agents_running():
    return subprocess.run(["pgrep", "-x", "agent"], capture_output=True,
                          text=True).stdout.split()


def start_agent(agent_id, log, no_fix):
    extra = {"AGENT_NO_ENSURE_TERMINAL": "1"} if no_fix else {}
    p = subprocess.Popen(
        [str(REPO / "target/debug/agent"), "--dds-domain", DOMAIN,
         "--engine", "dds", "--model", MODEL, "--agent-id", agent_id,
         "--heartbeat-ms", "1000"],
        stdout=open(log, "w"), stderr=subprocess.STDOUT,
        env=env(extra), start_new_session=True)
    return p


def kill_agents():
    for pid in agents_running():
        subprocess.run(["kill", "-9", pid])
    time.sleep(1.0)


def one_run(i, no_fix):
    kill_agents()
    a = start_agent("o4b-a", f"/var/tmp/o4b_a_{i}.log", no_fix)
    time.sleep(3.0)  # A assina Tasks
    t0 = time.time()
    client = subprocess.Popen(
        [SUBMIT, DOMAIN, MODEL,
         json.dumps([{"role": "user", "content": PROMPT}]),
         "--timeout-ms", "30000", "--max-tokens", "768",
         "--temperature", "0"],
        stdout=subprocess.PIPE, stderr=subprocess.DEVNULL, text=True,
        env=env())
    time.sleep(1.6)  # A está inferindo (TTFC ~0.5-1s)
    subprocess.run(["kill", "-9", str(a.pid)])
    b = start_agent("o4b-b", f"/var/tmp/o4b_b_{i}.log", no_fix)
    out, _ = client.communicate(timeout=60)
    wall = time.time() - t0
    line = (out or "").strip().splitlines()[-1] if (out or "").strip() else ""
    ok = False
    latency = None
    if line.startswith("{"):
        d = json.loads(line)
        ok = bool(d.get("success"))
        latency = d.get("latency_ms")
    print(f"run {i}: {'SUCESSO' if ok else 'FALHOU/timeout'} wall={wall:.1f}s "
          f"latency_ms={latency}", flush=True)
    return ok, wall, latency


def main():
    n = int(sys.argv[1])
    no_fix = "--no-fix" in sys.argv
    results = [one_run(i, no_fix) for i in range(1, n + 1)]
    kill_agents()
    oks = sum(1 for r in results if r[0])
    walls = [r[1] for r in results if r[0]]
    print(f"RESUMO no_fix={no_fix}: {oks}/{n} sucessos"
          + (f", wall média {sum(walls)/len(walls):.1f}s" if walls else ""))


if __name__ == "__main__":
    main()
