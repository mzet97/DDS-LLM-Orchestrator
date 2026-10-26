"""Cenário concorrência (REQ-907-006): c workflows simultâneos, isolamento por sessão.

Igual para os 3 sistemas: c execuções independentes do seq_chain_v1 com
workflow_ids distintos; mede makespan (fim - início global), latência por
workflow e conta falhas. Zero cross-talk = todos completam com 3 estágios e
conteúdo correto para o próprio id.
"""

from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass, field

from baselines.ports import LLMPort
from baselines.contracts import WorkflowInput
from baselines.systems.langgraph.seq import run_seq_langgraph
from baselines.systems.maf.seq import run_seq_maf


@dataclass
class ConcurrencyReport:
    system: str
    concurrency: int
    completed: int = 0
    failed: int = 0
    makespan_s: float = 0.0
    per_workflow_s: list[float] = field(default_factory=list)
    errors: list[str] = field(default_factory=list)


def _wf(case_id: str, workflow_id: str) -> WorkflowInput:
    return WorkflowInput(case_id=case_id, workflow_id=workflow_id,
                         workload_id="seq_chain_v1")


async def _gather(system: str, c: int, spawn) -> ConcurrencyReport:
    t0 = time.monotonic()

    async def one(i: int):
        s0 = time.monotonic()
        r = await spawn(i)
        return r, time.monotonic() - s0

    results = await asyncio.gather(*[one(i) for i in range(c)], return_exceptions=True)
    rep = ConcurrencyReport(system=system, concurrency=c, makespan_s=time.monotonic() - t0)
    for r in results:
        if isinstance(r, Exception):
            rep.failed += 1
            rep.errors.append(str(r)[:200])
        else:
            res, dt = r
            if res.status == "completed" and len(res.stages) == 3:
                rep.completed += 1
                rep.per_workflow_s.append(dt)
            else:
                rep.failed += 1
                rep.errors.append(f"status={res.status}")
    return rep


async def run_concurrency_langgraph(wf_base: WorkflowInput, port: LLMPort,
                                    prompts_dir: str, entry: str,
                                    c: int) -> ConcurrencyReport:
    async def spawn(i: int):
        return await run_seq_langgraph(_wf(wf_base.case_id, f"{wf_base.workflow_id}-{i}"),
                                       port, prompts_dir, entry)
    return await _gather("langgraph", c, spawn)


async def run_concurrency_maf(wf_base: WorkflowInput, port: LLMPort,
                              prompts_dir: str, entry: str,
                              c: int) -> ConcurrencyReport:
    async def spawn(i: int):
        return await run_seq_maf(_wf(wf_base.case_id, f"{wf_base.workflow_id}-{i}"),
                                 port, prompts_dir, entry)
    return await _gather("maf", c, spawn)
