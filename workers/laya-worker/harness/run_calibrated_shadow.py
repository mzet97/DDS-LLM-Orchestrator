#!/usr/bin/env python3
"""T-900-A6 — sessão shadow CALIBRADA (shadow_only=false observado).

Topologia (domínio isolado, loopback):
  1. wf-run --calibration-pin <pin> (gate ATIVO)
  2. laya-worker com --calibration-path (decisões carregam o pin)
  3. publica 1 intent
Gate: decisões aceitas com shadow_only=FALSE (pin casando) — sem publicar
Tasks (worker continua shadow).

Uso: run_calibrated_shadow.py <calibration.json> [--domain N]
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
ENV = {k: str(v) for k, v in os.environ.items()}
ENV["CYCLONEDDS_URI"] = f"file://{HERE / 'cyclonedds_loopback.xml'}"
os.environ["CYCLONEDDS_URI"] = ENV["CYCLONEDDS_URI"]
EVID = REPO / "specs/900-laya-adaptive-routing/evidence/calibration"

sys.path.insert(0, str(WORKER))


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("calibration", help="caminho do calibration_v1.json")
    ap.add_argument("--domain", type=int, default=80)
    args = ap.parse_args()

    import hashlib
    pin = hashlib.sha256(Path(args.calibration).read_bytes()).hexdigest()
    print(f"[A6] pin da calibração: {pin[:16]}…", flush=True)

    procs = {}
    timeline = []
    t0 = time.time()

    def mark(ev, **kw):
        timeline.append({"t": round(time.time() - t0, 1), "event": ev, **kw})
        print(f"[{timeline[-1]['t']:>6.1f}s] {ev} {kw}", flush=True)

    try:
        # 1) wf-run com --calibration-pin
        procs["wfrun"] = subprocess.Popen(
            [str(REPO / "target/debug/wf-observe"), "--domain", str(args.domain),
             "--expect", "1", "--timeout-s", "120",
             "--calibration-pin", pin],
            stdout=subprocess.PIPE, stderr=subprocess.STDOUT, text=True, env=ENV,
        )
        mark("wf-observe started (conta Tasks — deve ver ZERO)")

        # 2) worker REAL com calibration_path
        wenv = dict(ENV, PYTHONPATH=str(WORKER / "src"))
        procs["worker"] = subprocess.Popen(
            [str(VENV), "-m", "laya_worker", "--domain", str(args.domain),
             "--calibration-path", args.calibration,
             "--max-intents", "1", "--idle-timeout", "90"],
            cwd=str(WORKER), stdout=subprocess.PIPE, stderr=subprocess.STDOUT,
            text=True, env=wenv,
        )
        mark("laya-worker started (real + calibration)")

        # 3) publica intent
        time.sleep(8)
        from cyclonedds.domain import DomainParticipant
        from cyclonedds.pub import Publisher, DataWriter
        from cyclonedds.topic import Topic
        from cyclonedds.qos import Qos, Policy
        import dds_llm_workflow_v1 as w

        ONE_S = 10**9
        q = Qos(
            Policy.Reliability.Reliable(max_blocking_time=ONE_S),
            Policy.Durability.TransientLocal, Policy.History.KeepLast(1),
            Policy.ResourceLimits(max_samples=64, max_instances=64,
                                  max_samples_per_instance=1),
            Policy.Ownership.Shared,
        )
        dp = DomainParticipant(args.domain)
        dw = DataWriter(Publisher(dp),
                        Topic(dp, "Workflow.Intent", w.WorkflowIntent, qos=q), qos=q)
        now = time.time_ns()
        dw.write(w.WorkflowIntent(
            request_id="a6-1", workflow_id="wf-a6-1", client_id="a6",
            schema_version=1, intent_revision=1, command=0,
            expected_plan_revision=0, prompt="Resuma o relatorio trimestral",
            created_at_ns=now, deadline_ns=now + 300 * ONE_S,
            security_context_ref="", policy_revision=1, questions_revision=1,
            capability_catalog_revision=1, checkpoint_revision="",
            max_tokens=4096, max_cost_microunits=10**9,
            budget_unit="MICRO_USD", fallback_template_id="", payload_hash="",
        ))
        mark("intent publicada")

        # 4) espera wf-observe concluir
        obs_out, _ = procs["wfrun"].communicate(timeout=120)
        mark("wf-observe exited")
        shadow_false = "shadow_only=false" in obs_out or "shadow_only\":false" in obs_out.replace(" ", "")
        shadow_count = obs_out.count("shadow_only")
        tasks_seen = 0
        for line in obs_out.splitlines():
            if line.startswith("{") and "tasks_seen" in line:
                try:
                    tasks_seen = json.loads(line).get("tasks_seen", -1)
                except json.JSONDecodeError:
                    pass
        mark("wf-observe output",
             shadow_mentions=shadow_count, shadow_false=shadow_false,
             tasks_seen=tasks_seen)

        # worker stats
        wout, _ = procs["worker"].communicate(timeout=15)
        for line in (wout or "").splitlines():
            if line.startswith("{") and "tasks_written" in line:
                mark("worker stats", **json.loads(line))

        ok = shadow_false and tasks_seen == 0
        mark("A6 RESULTADO", PASS=ok,
             note="shadow_only=false exige pin casando + tasks=0 (fronteira)")
        EVID.mkdir(parents=True, exist_ok=True)
        (EVID / "calibrated-shadow.timeline.json").write_text(
            json.dumps(timeline, indent=1, ensure_ascii=False))
        return 0 if ok else 1
    finally:
        for p in procs.values():
            if p.poll() is None:
                p.terminate()
        time.sleep(2)
        for p in procs.values():
            if p.poll() is None:
                p.kill()


if __name__ == "__main__":
    sys.exit(main())
