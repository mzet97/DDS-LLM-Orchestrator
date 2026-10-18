"""Camada DDS do worker (T-900-M2-01).

Assina `Workflow.Intent` e publica `Routing.Decision` — NADA mais (D2/ADR-001:
sem writes em Tasks/TaskOutput/ToolCall). QoS idêntica aos perfis
`qos::profiles::workflow` do Rust (loopback config vem do invocador via
CYCLONEDDS_URI — ver harness/cyclonedds_loopback.xml).
"""

from __future__ import annotations

import sys
import time
from pathlib import Path
from typing import Callable, Iterator

_WORKER_ROOT = Path(__file__).resolve().parents[2]
if str(_WORKER_ROOT) not in sys.path:
    sys.path.insert(0, str(_WORKER_ROOT))

from cyclonedds.domain import DomainParticipant
from cyclonedds.pub import Publisher, DataWriter
from cyclonedds.sub import Subscriber, DataReader
from cyclonedds.topic import Topic
from cyclonedds.qos import Qos, Policy

import dds_llm_workflow_v1 as w

from . import decide as decide_mod

ONE_S = 1_000_000_000
TEN_S = 10_000_000_000


def _base_qos(n: int) -> Qos:
    return Qos(
        Policy.Reliability.Reliable(max_blocking_time=ONE_S),
        Policy.Durability.TransientLocal,
        Policy.History.KeepLast(1),
        Policy.ResourceLimits(max_samples=n, max_instances=n, max_samples_per_instance=1),
        Policy.Ownership.Shared,
    )


class WorkerSpace:
    """Entidades DDS mínimas do worker: sub intents, pub decisions."""

    def __init__(self, domain: int) -> None:
        self.dp = DomainParticipant(domain)
        self.pub = Publisher(self.dp)
        self.sub = Subscriber(self.dp)
        self.intent_reader = DataReader(
            self.sub,
            Topic(self.dp, "Workflow.Intent", w.WorkflowIntent, qos=_base_qos(64)),
            qos=_base_qos(64),
        )
        self.decision_writer = DataWriter(
            self.pub,
            Topic(self.dp, "Routing.Decision", w.RoutingDecision, qos=_base_qos(256)),
            qos=_base_qos(256),
        )

    def intents(self) -> Iterator[w.WorkflowIntent]:
        while True:
            batch = self.intent_reader.take()
            if not batch:
                return
            for sample in batch:
                # InvalidSample (dispose/gap) não tem campos: pular.
                if not hasattr(sample, "request_id"):
                    continue
                yield sample

    def publish_decision(self, d: dict) -> None:
        self.decision_writer.write(decide_mod.decision_to_wire(d, w))


def serve(space: WorkerSpace, laya, *, max_intents: int | None = None,
          poll_s: float = 0.05, idle_timeout_s: float | None = None) -> dict:
    """Loop shadow: decide cada intenção recebida; publica SOMENTE decisions.

    Retorna contadores (para o gate M2-04/V04: tasks_written == 0 sempre).
    """
    stats = {"intents_seen": 0, "decisions_ok": 0, "decisions_error": 0, "tasks_written": 0}
    started = time.monotonic()
    while max_intents is None or stats["intents_seen"] < max_intents:
        if idle_timeout_s is not None and time.monotonic() - started > idle_timeout_s:
            break
        got_any = False
        for intent in space.intents():
            got_any = True
            stats["intents_seen"] += 1
            d = decide_mod.decide(intent, laya)
            space.publish_decision(d)
            if d["decision_status"] == 0:
                stats["decisions_ok"] += 1
            else:
                stats["decisions_error"] += 1
        if not got_any:
            time.sleep(poll_s)
    return stats
