#!/usr/bin/env python3
"""T-900-M1-04 — assinante Python dos 5 tópicos da fase 900 (V02/V03).

QoS idêntica aos perfis Rust; poll com take() a cada 50 ms. Imprime uma linha
JSON por amostra recebida e sai 0 quando viu os 5 tópicos (ou 1 no timeout).
`--late` é semântico (documenta V03; o comportamento vem de quem inicia antes).
"""
import argparse
import json
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from cyclonedds.domain import DomainParticipant
from cyclonedds.sub import Subscriber, DataReader
from cyclonedds.topic import Topic
from cyclonedds.qos import Qos, Policy

import dds_llm_workflow_v1 as w

ONE_S = 1_000_000_000
TEN_S = 10_000_000_000


def base_qos(n: int) -> Qos:
    return Qos(
        Policy.Reliability.Reliable(max_blocking_time=ONE_S),
        Policy.Durability.TransientLocal,
        Policy.History.KeepLast(1),
        Policy.ResourceLimits(max_samples=n, max_instances=n, max_samples_per_instance=1),
        Policy.Ownership.Shared,
    )


def caps_qos() -> Qos:
    return Qos(
        *(base_qos(256).policies
          + (Policy.Deadline(TEN_S),
             Policy.Liveliness.ManualByTopic(lease_duration=TEN_S)))
    )


def report(topic: str, sample) -> None:
    if topic == "Workflow.Intent":
        line = {"topic": topic, "request_id": sample.request_id}
    elif topic == "Routing.Decision":
        line = {"topic": topic, "request_id": sample.request_id, "decision_id": sample.decision_id}
    elif topic == "Agent.Capabilities":
        line = {"topic": topic, "agent_id": sample.agent_id,
                "announcement_revision": sample.announcement_revision}
    elif topic == "Workflow.Plan":
        line = {"topic": topic, "workflow_id": sample.workflow_id,
                "plan_revision": sample.plan_revision, "nodes": len(sample.nodes)}
    else:
        line = {"topic": topic, "workflow_id": sample.workflow_id,
                "plan_revision": sample.plan_revision, "state_revision": sample.state_revision}
    print(json.dumps(line), flush=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--domain", type=int, default=211)
    ap.add_argument("--timeout-s", type=float, default=10.0)
    args = ap.parse_args()

    dp = DomainParticipant(args.domain)
    sub = Subscriber(dp)
    topics = [
        ("Workflow.Intent", w.WorkflowIntent, base_qos(64)),
        ("Routing.Decision", w.RoutingDecision, base_qos(256)),
        ("Agent.Capabilities", w.AgentCapabilities, caps_qos()),
        ("Workflow.Plan", w.WorkflowPlan, base_qos(32)),
        ("Workflow.State", w.WorkflowState, base_qos(32)),
    ]
    readers = {name: DataReader(sub, Topic(dp, name, ty, qos=q)) for name, ty, q in topics}

    seen: set[str] = set()
    deadline = time.monotonic() + args.timeout_s
    while time.monotonic() < deadline and len(seen) < 5:
        for name, reader in readers.items():
            for sample in reader.take():
                report(name, sample)
                seen.add(name)
        time.sleep(0.05)

    if len(seen) == 5:
        print('{"complete":true}', flush=True)
        return 0
    print(f'timeout: viu apenas {sorted(seen)}', file=sys.stderr, flush=True)
    return 1


if __name__ == "__main__":
    sys.exit(main())
