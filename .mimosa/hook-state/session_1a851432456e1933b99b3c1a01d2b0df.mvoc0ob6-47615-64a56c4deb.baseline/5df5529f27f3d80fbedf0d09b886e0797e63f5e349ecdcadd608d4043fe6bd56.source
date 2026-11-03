#!/usr/bin/env python3
"""T-904-M5 — shadow REAL do laya-worker com o checkpoint T900 (DDS real).

Topologia (domínio isolado, config loopback igual em todos os processos):
  wf-observe (Rust)  — valida cada decisão vs a intenção; conta Tasks (gate: 0);
                       --calibration-pin <pin do BASE> prova que decisão sem a
                       calibração correspondente NUNCA sai de shadow;
  laya-worker        — RealLayaAdapter com --checkpoint-path (T900 35813054…,
                       hash verificado no start) + --calibration-path <calib BASE>
                       para CENÁRIO de divergência? NÃO: default sem calibração
                       (calibration_revision="");
  este script        — publica 5 intents reais + 3 decisões FORJADAS (casos de
                       rejeição do validator) e captura as decisões publicadas.

Cenários (intent → resultado esperado no observador):
  t904-ok        pin=T900        → accept shadow_only=true
  t904-nopin     pin=""          → accept shadow_only=true
  t904-empty     prompt=""       → observado (entrada degenerada)
  t904-oldrev    questions_rev=2 → error reason_code=5 (RC_SCHEMA_MISMATCH)
  t904-expired   deadline passado→ error reason_code=6 (RC_TIMEOUT)
  t904-badpin    (forjada) checkpoint=BASE ≠ pin da intenção → reject RevisionPin
  t904-stale     (forjada) valid_until no passado → reject ValidityWindow
  t904-badcal    (forjada) calibration pin aleatório ≠ policy → accept shadow_only
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
T900_SHA = "358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f"
BASE_SHA = "891102d372688fc2a094dac56a384bc537b87c63f21f9f3dac0be2b7cbc8d86c"
CALIB_BASE_PIN = "10ef5945842c7855b941c7a9e46a823ae2d0801e5cc2b8751cf7d865e947b6e5"
CHECKPOINT_DIR = os.path.expanduser("~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1")
LAYA_PY = os.path.expanduser("~/.cache/tese-laya/venv/bin/python")

ENV = dict(os.environ, CYCLONEDDS_URI=f"file://{HERE / 'cyclonedds_loopback.xml'}")
ENV["PYTHONPATH"] = str(WORKER / "src") + os.pathsep + ENV.get("PYTHONPATH", "")
# O PRÓPRIO processo publica/assina DDS — precisa do MESMO isolamento loopback
# dos filhos, senão o tráfego vaza para a LAN do lab e os transportes não casam.
os.environ["CYCLONEDDS_URI"] = ENV["CYCLONEDDS_URI"]

sys.path.insert(0, str(WORKER))
from cyclonedds.domain import DomainParticipant  # noqa: E402
from cyclonedds.pub import Publisher, DataWriter  # noqa: E402
from cyclonedds.sub import Subscriber, DataReader  # noqa: E402
from cyclonedds.topic import Topic  # noqa: E402
from cyclonedds.qos import Qos, Policy  # noqa: E402

import dds_llm_workflow_v1 as w  # noqa: E402

ONE_S = 1_000_000_000
PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]


def base_qos(n: int) -> Qos:
    return Qos(
        Policy.Reliability.Reliable(max_blocking_time=ONE_S),
        Policy.Durability.TransientLocal,
        Policy.History.KeepLast(1),
        Policy.ResourceLimits(max_samples=n, max_instances=n, max_samples_per_instance=1),
        Policy.Ownership.Shared,
    )


def intent(request_id: str, *, checkpoint_pin: str, questions_revision: int = 1,
           deadline_ns: int | None = None, prompt: str = "streaming live captions with partial results") -> w.WorkflowIntent:
    now = time.time_ns()
    return w.WorkflowIntent(
        request_id=request_id, workflow_id="wf-t904", client_id="harness-t904",
        schema_version=1, intent_revision=1, command=w.IntentCommand.IC_SUBMIT.value,
        expected_plan_revision=0, prompt=prompt,
        created_at_ns=now,
        deadline_ns=deadline_ns if deadline_ns is not None else now + 600 * ONE_S,
        security_context_ref="ctx", policy_revision=1,
        questions_revision=questions_revision, capability_catalog_revision=1,
        checkpoint_revision=checkpoint_pin,
        max_tokens=10, max_cost_microunits=0, budget_unit="MICRO_USD",
        fallback_template_id="", payload_hash="ph")


def forged_decision(request_id: str, *, checkpoint_revision: str,
                    calibration_revision: str, valid_until_ns: int) -> w.RoutingDecision:
    """Decisão SINTÉTICA consistente exceto pela divergência sob teste."""
    now = time.time_ns()
    dist = [w.ProfileProbability(i, p) for i, p in enumerate([0.1, 0.9, 0.0, 0.0, 0.0])]
    return w.RoutingDecision(
        request_id=request_id, decision_id=f"dec-{request_id}", workflow_id="wf-t904",
        schema_version=1, intent_revision=1, producer_id="harness-forge",
        model_id="forged", checkpoint_revision=checkpoint_revision,
        questions_revision=1, policy_revision=1, capability_catalog_revision=1,
        qos_profile=w.QosIntent.QI_CRITICAL.value, qos_probabilities=dist,
        capability_probabilities=[], requires_planning_probability=0.2,
        priority_score=1.5, priority_probabilities=[0.1, 0.7, 0.2], confidence=0.8,
        answer_confidence=0.7, calibration_revision=calibration_revision,
        created_at_ns=now, valid_until_ns=valid_until_ns,
        decision_status=w.DecisionStatus.DS_OK.value,
        reason_code=w.ReasonCode.RC_NONE.value, evidence_digest="e", payload_hash="dh")


def decision_to_dict(s) -> dict:
    return {
        "request_id": s.request_id, "decision_id": s.decision_id,
        "model_id": s.model_id, "checkpoint_revision": s.checkpoint_revision,
        "calibration_revision": s.calibration_revision,
        "qos_profile": int(s.qos_profile),
        "decision_status": int(s.decision_status), "reason_code": int(s.reason_code),
        "confidence": float(s.confidence),
        "created_at_ns": int(s.created_at_ns), "valid_until_ns": int(s.valid_until_ns),
    }


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--domain", type=int, default=231)
    ap.add_argument("--hold-s", type=float, default=8.0)
    ap.add_argument("--checkpoint-dir", default=CHECKPOINT_DIR,
                    help="diretório do checkpoint a carregar (default: T900)")
    ap.add_argument("--checkpoint-sha", default=T900_SHA)
    ap.add_argument("--calibration-path", default=None,
                    help="calibração a vincular (pin vira calibration_revision)")
    ap.add_argument("--observe-pin", default=CALIB_BASE_PIN,
                    help="pin esperado pelo wf-observe (calibração válida)")
    args = ap.parse_args()

    observe = subprocess.Popen(
        [str(Path(os.environ.get("T904_OBSERVE", "/tmp/tese-laya-t904-target/debug/wf-observe"))),
         "--domain", str(args.domain), "--expect", "8", "--timeout-s", "180",
         "--calibration-pin", args.observe_pin],
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=ENV)
    time.sleep(1.5)
    worker = subprocess.Popen(
        [LAYA_PY, "-m", "laya_worker", "--domain", str(args.domain),
         "--checkpoint-path", args.checkpoint_dir, "--checkpoint-sha256", args.checkpoint_sha,
         "--max-intents", "5", "--idle-timeout", "150"]
        + (["--calibration-path", args.calibration_path] if args.calibration_path else []),
        stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True, env=ENV)
    time.sleep(4.0)  # discovery + adapter start (hash pré-load no __init__)

    dp = DomainParticipant(args.domain)
    pub = Publisher(dp)
    sub = Subscriber(dp)
    intent_w = DataWriter(pub, Topic(dp, "Workflow.Intent", w.WorkflowIntent, qos=base_qos(64)))
    forged_w = DataWriter(pub, Topic(dp, "Routing.Decision", w.RoutingDecision, qos=base_qos(256)))
    decision_r = DataReader(sub, Topic(dp, "Routing.Decision", w.RoutingDecision, qos=base_qos(256)))

    now = time.time_ns()
    intents = {
        "t904-ok": intent("t904-ok", checkpoint_pin=args.checkpoint_sha),
        "t904-nopin": intent("t904-nopin", checkpoint_pin=""),
        "t904-empty": intent("t904-empty", checkpoint_pin=args.checkpoint_sha, prompt=""),
        "t904-oldrev": intent("t904-oldrev", checkpoint_pin=args.checkpoint_sha, questions_revision=2),
        "t904-expired": intent("t904-expired", checkpoint_pin=args.checkpoint_sha, deadline_ns=now - 10 * ONE_S),
    }
    for rid, it in intents.items():
        intent_w.write(it)
        print(json.dumps({"published": "Workflow.Intent", "request_id": rid}), flush=True)

    published = []
    deadline = time.monotonic() + 120
    while len(published) < 5 and time.monotonic() < deadline:
        samples = decision_r.take()
        for s in samples:
            if not hasattr(s, "request_id"):
                continue
            published.append(decision_to_dict(s))
            print(json.dumps({"decision": published[-1]}, ensure_ascii=False), flush=True)
        time.sleep(0.2)

    # fases forjadas (validator Rust; worker não participa)
    now = time.time_ns()
    forged = [
        ("t904-badpin", intent("t904-badpin", checkpoint_pin=args.checkpoint_sha),
         forged_decision("t904-badpin", checkpoint_revision=BASE_SHA,
                         calibration_revision="", valid_until_ns=now + 600 * ONE_S)),
        ("t904-stale", intent("t904-stale", checkpoint_pin=args.checkpoint_sha),
         forged_decision("t904-stale", checkpoint_revision=T900_SHA,
                         calibration_revision="", valid_until_ns=now - 60 * ONE_S)),
        ("t904-badcal", intent("t904-badcal", checkpoint_pin=args.checkpoint_sha),
         forged_decision("t904-badcal", checkpoint_revision=T900_SHA,
                         calibration_revision="f" * 64, valid_until_ns=now + 600 * ONE_S)),
    ]
    for rid, it, dec in forged:
        intent_w.write(it)
        forged_w.write(dec)
        print(json.dumps({"published": "forged", "request_id": rid}), flush=True)

    deadline = time.monotonic() + 20
    while len(published) < 8 and time.monotonic() < deadline:
        samples = decision_r.take()
        for s in samples:
            if not hasattr(s, "request_id"):
                continue
            published.append(decision_to_dict(s))
            print(json.dumps({"decision": published[-1]}, ensure_ascii=False), flush=True)
        time.sleep(0.2)

    time.sleep(args.hold_s)
    try:
        worker_out, worker_err = worker.communicate(timeout=20)
    except subprocess.TimeoutExpired:
        worker.kill()
        worker_out, worker_err = worker.communicate()
    observe_out, observe_err = observe.communicate(timeout=60)

    result = {
        "domain": args.domain,
        "decisions_published": published,
        "worker_stdout": (worker_out or "").strip()[-800:],
        "worker_stderr": (worker_err or "").strip()[-500:],
        "wf_observe_stdout": observe_out.strip(),
        "wf_observe_stderr": observe_err.strip()[-500:],
    }
    print(json.dumps(result, ensure_ascii=False, indent=1))
    return 0


if __name__ == "__main__":
    sys.exit(main())
