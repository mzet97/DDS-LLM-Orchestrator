#!/usr/bin/env python3
"""T-900-M1-04 — publicador Python dos 5 tópicos da fase 900 (V02/V03).

Usa os bindings gerados (`../dds_llm_workflow_v1`) e QoS IDÊNTICA aos perfis
`qos::profiles::workflow` do Rust ( Reliable(1s)/TransientLocal/KeepLast(1)/
Shared/ResourceLimits(N,N,1); Capabilities + Deadline 10s + ManualByTopic 10s)
— sem isso o matching requested/offered não acontece e o teste não valida.

Modos:
  --wrong-type  publica um RoutingDecision NO tópico Workflow.Intent
                 (incompatibilidade deliberada do V02: NÃO deve ser entregue).
  --hold-s S     mantém o processo vivo S segundos após publicar (V03:
                 durability TransientLocal para late joiner).
"""
import argparse
import sys
import time
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent.parent))

from cyclonedds.domain import DomainParticipant
from cyclonedds.pub import Publisher, DataWriter
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


def intent(i: int) -> w.WorkflowIntent:
    return w.WorkflowIntent(
        request_id=f"v02-intent-{i}", workflow_id="wf-v02", client_id="harness",
        schema_version=1, intent_revision=1, command=w.IntentCommand.IC_SUBMIT.value,
        expected_plan_revision=0, prompt="interop probe m1-04",
        created_at_ns=time.time_ns(), deadline_ns=time.time_ns() + 600_000_000_000,
        security_context_ref="ctx", policy_revision=1,
        questions_revision=1, capability_catalog_revision=1, checkpoint_revision="",
        max_tokens=10, max_cost_microunits=0, budget_unit="MICRO_USD",
        fallback_template_id="", payload_hash="ph")


def decision(req: str) -> w.RoutingDecision:
    return w.RoutingDecision(
        request_id=req, decision_id="dec-v02", workflow_id="wf-v02", schema_version=1,
        intent_revision=1, producer_id="harness", model_id="laya", checkpoint_revision="a",
        questions_revision=1, policy_revision=1, capability_catalog_revision=1,
        qos_profile=w.QosIntent.QI_CRITICAL.value,
        qos_probabilities=[w.ProfileProbability(w.QosIntent.QI_CRITICAL.value, 0.9)],
        capability_probabilities=[], requires_planning_probability=0.2,
        priority_score=1.5, priority_probabilities=[0.1, 0.7, 0.2], confidence=0.8,
        answer_confidence=0.7, calibration_revision="", created_at_ns=1,
        valid_until_ns=2, decision_status=w.DecisionStatus.DS_OK.value,
        reason_code=w.ReasonCode.RC_NONE.value, evidence_digest="e", payload_hash="dh")


def caps() -> w.AgentCapabilities:
    return w.AgentCapabilities(
        agent_id="v02-agent", schema_version=1, announcement_revision=1,
        capability_catalog_revision=1, capabilities=["rust_refactor", "unit_testing"],
        model_ids=["gemma-4-12b-it"], tool_versions=[], adaptive_protocol_version=1,
        security_context_ref="ctx", created_at_ns=1, valid_until_ns=11)


def plan() -> w.WorkflowPlan:
    node = w.PlanNode(
        node_id="node-a", kind=w.NodeKind.NK_INFERENCE.value,
        required_capabilities=["rust_refactor"], model_id="gemma-4-12b-it",
        target_agent_id="", prompt_template="probe", dependencies=[], inputs=[],
        output_names=["out"], max_attempts=2, deadline_ns=3, max_tokens=64,
        max_output_bytes=1024, security_context_ref="ctx", priority=5,
        side_effect_key="")
    return w.WorkflowPlan(
        workflow_id="v02-wf", plan_revision=1, schema_version=1, request_id="req-v02",
        intent_revision=1, effective_decision_id="dec-v02", policy_revision=1,
        capability_catalog_revision=1, planner_id="static-template", planner_revision="r",
        created_at_ns=1, deadline_ns=3, qos_profile=w.QosIntent.QI_CRITICAL.value,
        nodes=[node], plan_hash="planhash")


def state() -> w.WorkflowState:
    return w.WorkflowState(
        workflow_id="v02-wf", plan_revision=1, schema_version=1, state_revision=1,
        request_id="req-v02", effective_decision_id="dec-v02", plan_hash="planhash",
        status=w.WorkflowStatus.WS_RUNNING.value, reason_code=w.ReasonCode.RC_NONE.value,
        nodes=[], created_at_ns=1, updated_at_ns=1, expires_at_ns=99,
        cancelled_by_request_id="", effective_qos_config_hash="qoshash",
        fallback_template_id="")


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--domain", type=int, default=211)
    ap.add_argument("--count", type=int, default=1)
    ap.add_argument("--wrong-type", action="store_true")
    ap.add_argument("--hold-s", type=float, default=0.0)
    ap.add_argument("--only-intent", action="store_true",
                    help="publica SOMENTE Workflow.Intent (shadow M2-04)")
    args = ap.parse_args()

    dp = DomainParticipant(args.domain)
    pub = Publisher(dp)

    if args.wrong_type:
        # Incompatibilidade deliberada: RoutingDecision no tópico Workflow.Intent.
        t = Topic(dp, "Workflow.Intent", w.RoutingDecision, qos=base_qos(64))
        dw = DataWriter(pub, t)
        dw.write(decision("wrong-type-probe"))
        print('{"published":"Workflow.Intent(WRONG-TYPE:RoutingDecision)"}', flush=True)
        time.sleep(args.hold_s)
        return 0

    if args.only_intent:
        t = Topic(dp, "Workflow.Intent", w.WorkflowIntent, qos=base_qos(64))
        dw = DataWriter(pub, t, qos=base_qos(64))
        for i in range(args.count):
            dw.write(intent(i))
            print(f'{{"published":"Workflow.Intent","request_id":"v02-intent-{i}"}}', flush=True)
        if args.hold_s > 0:
            time.sleep(args.hold_s)
        return 0

    topics = [
        ("Workflow.Intent", w.WorkflowIntent, base_qos(64)),
        ("Routing.Decision", w.RoutingDecision, base_qos(256)),
        ("Agent.Capabilities", w.AgentCapabilities, caps_qos()),
        ("Workflow.Plan", w.WorkflowPlan, base_qos(32)),
        ("Workflow.State", w.WorkflowState, base_qos(32)),
    ]
    writers = {name: DataWriter(pub, Topic(dp, name, ty, qos=q)) for name, ty, q in topics}

    for i in range(args.count):
        writers["Workflow.Intent"].write(intent(i))
        print(f'{{"published":"Workflow.Intent","request_id":"v02-intent-{i}"}}', flush=True)
        writers["Routing.Decision"].write(decision(f"v02-intent-{i}"))
        print('{"published":"Routing.Decision","request_id":"'
              f'v02-intent-{i}","decision_id":"dec-v02"}}', flush=True)
        writers["Agent.Capabilities"].write(caps())
        print('{"published":"Agent.Capabilities","agent_id":"v02-agent"}', flush=True)
        writers["Workflow.Plan"].write(plan())
        print('{"published":"Workflow.Plan","workflow_id":"v02-wf","plan_revision":1}', flush=True)
        writers["Workflow.State"].write(state())
        print('{"published":"Workflow.State","workflow_id":"v02-wf","state_revision":1}', flush=True)

    if args.hold_s > 0:
        time.sleep(args.hold_s)
    return 0


if __name__ == "__main__":
    sys.exit(main())
