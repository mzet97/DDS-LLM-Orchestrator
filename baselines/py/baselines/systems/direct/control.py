"""Controles diretos sem orquestrador (Prompt §16.3): mesma API de inferência."""

from __future__ import annotations

import asyncio
import json
import urllib.request

from baselines.contracts import StageInput, StageOutput, WorkflowInput, WorkflowResult
from baselines.systems.assembly import (ANALYSIS, CORRECTNESS, REVIEW, SECURITY,
                                     build_messages, load_prompt)


def _call(base_url: str, messages: list[dict], stage: str, expects: list[str]) -> StageOutput:
    payload = {"messages": messages,
               "bench": {"stage": stage, "expects": expects}}
    req = urllib.request.Request(base_url.rstrip("/") + "/v1/chat/completions",
                                 data=json.dumps(payload).encode(),
                                 headers={"Content-Type": "application/json"})
    with urllib.request.urlopen(req) as r:
        body = json.loads(r.read())
    content = body["choices"][0]["message"]["content"]
    return StageOutput(request_id=f"{stage}-direct", step_id=stage, content=content)


def run_seq_direct(wf: WorkflowInput, base_url: str, prompts_dir: str,
                   entry: str) -> WorkflowResult:
    """Sequência A→B→C com chamadas explícitas ordenadas (controle, não baseline).

    Usa a MESMA regra de montagem dos baselines (assembly.build_messages).
    """
    ma = build_messages(load_prompt(prompts_dir, "seq_A_analyst_v1.txt"), entry)
    sa = _call(base_url, ma, "A", [])
    mb = build_messages(load_prompt(prompts_dir, "seq_B_reviewer_v1.txt"), entry,
                        ((ANALYSIS, sa.content),))
    sb = _call(base_url, mb, "B", [sa.content])
    mc = build_messages(load_prompt(prompts_dir, "seq_C_consolidator_v1.txt"), entry,
                        ((ANALYSIS, sa.content), (REVIEW, sb.content)))
    sc = _call(base_url, mc, "C", [sa.content, sb.content])
    return WorkflowResult(workflow_id=wf.workflow_id, workload_id=wf.workload_id,
                          status="completed", stages=(sa, sb, sc))


async def run_forkjoin_direct(wf: WorkflowInput, base_url: str, prompts_dir: str,
                              entry: str) -> WorkflowResult:
    """Fork-join com sobreposição real via asyncio (controle paralelo).

    Mesma regra de montagem dos baselines; C recebe A/B em ordem canônica.
    """
    loop = asyncio.get_running_loop()
    ma = build_messages(load_prompt(prompts_dir, "fork_A_correctness_v1.txt"), entry)
    mb = build_messages(load_prompt(prompts_dir, "fork_B_security_v1.txt"), entry)

    def call(m, stage, expects):
        return _call(base_url, m, stage, expects)

    sa, sb = await asyncio.gather(
        loop.run_in_executor(None, call, ma, "A", []),
        loop.run_in_executor(None, call, mb, "B", []),
    )
    mc = build_messages(load_prompt(prompts_dir, "fork_C_consolidator_v1.txt"), entry,
                        ((CORRECTNESS, sa.content), (SECURITY, sb.content)))
    sc = _call(base_url, mc, "C", [sa.content, sb.content])
    return WorkflowResult(workflow_id=wf.workflow_id, workload_id=wf.workload_id,
                          status="completed", stages=(sa, sb, sc))


def build_stage_input(wf: WorkflowInput, step: str, n: int,
                      messages: list[dict]) -> StageInput:
    return StageInput(workflow_id=wf.workflow_id, step_id=step, attempt_id=1,
                      request_id=f"{wf.workflow_id}-{step}-1", agent_role=step,
                      agent_instance_id=f"{step}-direct-0", messages=tuple(messages))
