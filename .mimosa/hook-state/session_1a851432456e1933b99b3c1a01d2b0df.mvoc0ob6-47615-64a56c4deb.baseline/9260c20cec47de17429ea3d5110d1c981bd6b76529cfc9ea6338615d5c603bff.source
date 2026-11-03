#!/usr/bin/env python3
"""T-900-E2E-B3 — harness ponta-a-ponta do braço híbrido (DDS real).

Topologia (domínio único 216, config loopback, processos gerenciados por ESTE
script — nada de fundo entre chamadas do harness):
  1. llama-server ROCm (gemma-4-12b UD-Q4_K_XL oficial) na 8901
  2. wf-run --planner llm (engine: journal + scheduler + materializer)
  3. laya-worker (adapter REAL; decide as intents)
  4. agent --adaptive --engine http --llama-url 127.0.0.1:8901 (executa as
     tasks laya1- via inferência real, escreve DONE)
  5. publica N intents mistas (PT simples / EN multi-etapa)
Gate: cada workflow termina DONE no wf-run; timeline JSON com marcos.
"""

import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORKER = HERE.parent
REPO = WORKER.parent.parent
VENV = Path.home() / ".cache/tese-laya/venv/bin/python"
LLAMA_BIN = Path.home() / ".cache/tese-benchmarks/llama-prebuilt/llama-b11671/llama-server"
MODEL = Path("/var/mnt/HD1TB/tese/models/gemma-4-12b-it-UD-Q4_K_XL.gguf")
ENV = {k: str(v) for k, v in os.environ.items()}
ENV["CYCLONEDDS_URI"] = f"file://{HERE / 'cyclonedds_loopback.xml'}"
# o processo que PUBLICA também precisa do loopback (a config do host não
# descobre processos Python — ver evidence/m1-04)
os.environ["CYCLONEDDS_URI"] = ENV["CYCLONEDDS_URI"]


def wait_health(port: int, timeout_s: int = 240) -> bool:
    import urllib.request
    for _ in range(timeout_s // 5):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=3) as r:
                if b"ok" in r.read():
                    return True
        except Exception:
            pass
        time.sleep(5)
    return False


_PUB_STATE = {}


def publish_intent(domain: int, request_id: str, prompt: str, ttl_budget: int = 10**9) -> None:
    """TransientLocal exige o writer VIVO até o leitor receber: participantes/
    writers ficam retidos em _PUB_STATE (GC do Python destruiria o writer e
    perderia a amostra retida)."""
    sys.path.insert(0, str(WORKER))
    from cyclonedds.domain import DomainParticipant
    from cyclonedds.pub import Publisher, DataWriter
    from cyclonedds.topic import Topic
    from cyclonedds.qos import Qos, Policy
    import dds_llm_workflow_v1 as w

    ONE_S = 10**9
    q = Qos(
        Policy.Reliability.Reliable(max_blocking_time=ONE_S),
        Policy.Durability.TransientLocal,
        Policy.History.KeepLast(1),
        Policy.ResourceLimits(max_samples=64, max_instances=64, max_samples_per_instance=1),
        Policy.Ownership.Shared,
    )
    if "dw" not in _PUB_STATE:
        dp = DomainParticipant(domain)
        pub = Publisher(dp)
        t = Topic(dp, "Workflow.Intent", w.WorkflowIntent, qos=q)
        _PUB_STATE["dp"] = dp
        _PUB_STATE["dw"] = DataWriter(pub, t, qos=q)
        time.sleep(1.5)  # descoberta antes da primeira escrita
    dw = _PUB_STATE["dw"]
    now = time.time_ns()
    intent = w.WorkflowIntent(
        request_id=request_id, workflow_id=f"wf-{request_id}", client_id="e2e",
        schema_version=1, intent_revision=1, command=w.IntentCommand.IC_SUBMIT.value,
        expected_plan_revision=0, prompt=prompt, created_at_ns=now,
        deadline_ns=now + 900_000_000_000, security_context_ref="", policy_revision=1,
        questions_revision=1, capability_catalog_revision=1, checkpoint_revision="",
        max_tokens=4096, max_cost_microunits=10**9, budget_unit="MICRO_USD",
        fallback_template_id="", payload_hash="",
    )
    dw.write(intent)
    print(f"[pub] intent {request_id}: {prompt[:60]}...", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--domain", type=int, default=216)
    ap.add_argument("--port", type=int, default=8901)
    ap.add_argument("--journal", default="/tmp/wf-e2e-journal")
    args = ap.parse_args()

    import shutil
    shutil.rmtree(args.journal, ignore_errors=True)

    procs = {}
    timeline = []
    t0 = time.time()

    def mark(ev: str, **kw):
        timeline.append({"t": round(time.time() - t0, 1), "event": ev, **kw})
        print(f"[{timeline[-1]['t']:>6.1f}s] {ev} {kw}", flush=True)

    try:
        # 1) llama-server
        mark("llama-server starting")
        procs["llama"] = subprocess.Popen(
            [str(LLAMA_BIN), "-m", str(MODEL), "-ngl", "99", "-np", "1", "-c", "4096",
             "--host", "127.0.0.1", "--port", str(args.port)],
            stdout=open("/tmp/e2e-llama.log", "w"), stderr=subprocess.STDOUT, env=ENV,
        )
        if not wait_health(args.port):
            mark("llama-server FAILED health")
            return 2
        mark("llama-server healthy")

        # 2) wf-run (engine)
        procs["wfrun"] = subprocess.Popen(
            [str(REPO / "target/debug/wf-run"), "--domain", str(args.domain),
             "--planner", "llm", "--llama-url", f"http://127.0.0.1:{args.port}",
             "--journal-dir", args.journal, "--expect", "2", "--timeout-s", "420"],
            stdout=open("/tmp/e2e-wfrun.log", "w"), stderr=subprocess.STDOUT, env=ENV,
        )
        mark("wf-run started")

        # 3) laya-worker REAL
        wenv = dict(ENV, PYTHONPATH=str(WORKER / "src"))
        procs["worker"] = subprocess.Popen(
            [str(VENV), "-m", "laya_worker", "--domain", str(args.domain),
             "--max-intents", "2", "--idle-timeout", "360"],
            cwd=str(WORKER), stdout=open("/tmp/e2e-worker.log", "w"),
            stderr=subprocess.STDOUT, env=wenv,
        )
        mark("laya-worker started (real)")

        # 4) agent adaptativo com engine http
        procs["agent"] = subprocess.Popen(
            [str(REPO / "target/debug/agent"), "--agent-id", "e2e-agent-1",
             "--dds-domain", str(args.domain), "--engine", "http",
             "--llama-url", f"http://127.0.0.1:{args.port}", "--slots", "2",
             "--adaptive", "--adaptive-domain", str(args.domain),
             "--adaptive-capabilities", "summarization,research,translation,rust_refactor"],
            stdout=open("/tmp/e2e-agent.log", "w"), stderr=subprocess.STDOUT, env=ENV,
        )
        mark("agent started (--adaptive, http engine)")

        time.sleep(6)  # descoberta
        # 5) intents
        publish_intent(args.domain, "e2e-1", "Resuma o relatório trimestral em cinco tópicos")
        publish_intent(args.domain, "e2e-2", "First research the outage timeline, then write the incident report")
        mark("2 intents published")

        # espera o wf-run concluir (exit 0 = workflows DONE)
        rc = procs["wfrun"].wait(timeout=420)
        mark("wf-run exited", rc=rc)
        out = Path("/tmp/e2e-wfrun.log").read_text()
        for line in out.splitlines():
            if line.startswith("{"):
                mark("wf-run result", **json.loads(line))

        ok = rc == 0
        # worker stats: stdout foi para /tmp/e2e-worker.log
        try:
            procs["worker"].wait(timeout=10)
        except subprocess.TimeoutExpired:
            procs["worker"].kill()
        for line in Path("/tmp/e2e-worker.log").read_text().splitlines():
            if line.startswith("{") and "tasks_written" in line:
                mark("worker stats", **json.loads(line))
                ok = ok and json.loads(line).get("tasks_written") == 0
        # agent log: claim/execução
        agent_log = Path("/tmp/e2e-agent.log").read_text()
        claims = agent_log.count("claim confirmado") + agent_log.count("ASSIGNED")
        mark("agent log", claims=claims, lines=len(agent_log.splitlines()))

        mark("E2E RESULTADO", PASS=ok)
        return 0 if ok else 1
    finally:
        for name, p in procs.items():
            if p.poll() is None:
                p.terminate()
        time.sleep(3)
        for name, p in procs.items():
            if p.poll() is None:
                p.kill()
        outdir = Path("/var/mnt/HD1TB/tese/src/rust/specs/900-laya-adaptive-routing/evidence/m6/e2e")
        outdir.mkdir(parents=True, exist_ok=True)
        (outdir / "timeline.json").write_text(json.dumps(timeline, indent=1, ensure_ascii=False))
        for name in ("llama", "wfrun", "worker", "agent"):
            src = Path(f"/tmp/e2e-{name}.log")
            if src.exists():
                (outdir / f"{name}.log").write_text(src.read_text()[:200_000])


if __name__ == "__main__":
    sys.exit(main())
