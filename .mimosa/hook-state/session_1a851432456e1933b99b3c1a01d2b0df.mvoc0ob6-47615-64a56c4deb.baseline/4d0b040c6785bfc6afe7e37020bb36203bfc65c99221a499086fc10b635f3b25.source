#!/usr/bin/env python3
"""T-900-M2-04 / V04 — sessão shadow real do laya-worker (mock adapter).

Topologia (domínio isolado, config loopback):
  py pub --only-intent (3 intenções, hold 10s)
  laya-worker --mock   (assina intents, publica Routing.Decision; NUNCA Tasks)
  wf-observe (Rust)    (valida decisões; conta Tasks — deve ver ZERO)

Gate V04: decisões validadas >=3, tasks_seen == 0, exit codes 0.
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
ROOT = WORKER.parent.parent
ENV = dict(os.environ, CYCLONEDDS_URI=f"file://{HERE / 'cyclonedds_loopback.xml'}")
# layout src/: o worker roda sem instalação via PYTHONPATH
ENV["PYTHONPATH"] = str(WORKER / "src") + os.pathsep + ENV.get("PYTHONPATH", "")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--domain", type=int, default=213)
    ap.add_argument("--intents", type=int, default=3)
    args = ap.parse_args()

    observe = subprocess.Popen(
        [str(ROOT / "target/debug/wf-observe"), "--domain", str(args.domain),
         "--expect", str(args.intents), "--timeout-s", "40"],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=ENV,
    )
    time.sleep(1.5)
    worker = subprocess.Popen(
        [sys.executable, "-m", "laya_worker", "--domain", str(args.domain),
         "--mock", "--max-intents", str(args.intents), "--idle-timeout", "20"],
        cwd=str(WORKER), stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=ENV,
    )
    time.sleep(1.5)
    pub = subprocess.run(
        [sys.executable, str(HERE / "py_workflow_pub.py"), "--domain", str(args.domain),
         "--only-intent", "--count", str(args.intents), "--hold-s", "8"],
        capture_output=True, text=True, env=ENV, timeout=30,
    )
    print("pub:", pub.stdout.strip().replace("\n", " | "))
    if pub.returncode != 0:
        print("pub STDERR:", pub.stderr[-400:])
        observe.kill()
        worker.kill()
        return 2

    w_out, w_err = worker.communicate(timeout=35)
    print("worker:", (w_out or "").strip())
    if w_err.strip():
        print("worker stderr:", w_err[-300:])
    o_out, o_err = observe.communicate(timeout=45)
    print("observe:", (o_out or "").strip())
    if o_err.strip():
        print("observe stderr:", o_err[-300:])

    stats = {}
    for line in (o_out or "").splitlines():
        if line.startswith("{") and "decisions" in line:
            try:
                stats = json.loads(line)
            except json.JSONDecodeError:
                pass
    ok = (
        worker.returncode == 0
        and observe.returncode == 0
        and stats.get("decisions", 0) >= args.intents
        and stats.get("accepted", 0) + stats.get("errors", 0) >= args.intents
        and stats.get("tasks_seen") == 0
    )
    print("V04 RESULTADO:", "PASS" if ok else "FAIL", stats)
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
