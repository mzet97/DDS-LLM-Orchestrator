"""Cenário retry (REQ-907-005): falha transiente 1× reposta dentro do teto.

- LangGraph: `RetryPolicy` nativa do grafo (declarativa, por nó).
- MAF: re-execução do executor (subclasse com laço de retry no handler) — o
  framework 1.18 não expõe política de retry declarativa em arestas; isto é
  documentado como análogo, não idêntico (matriz da spec).
"""

from __future__ import annotations

import asyncio
import time
from dataclasses import dataclass, field

from agent_framework import WorkflowContext, handler
from langgraph.types import RetryPolicy

from baselines.ports import LLMPort
from baselines.contracts import StageOutput, WorkflowInput
from baselines.systems.langgraph.seq import build_seq_graph, run_seq_langgraph
from baselines.systems.maf.seq import (OutA, OutB, OutC, StageExecutor,
                                       build_seq_workflow)
from baselines.systems.assembly import (ANALYSIS, REVIEW, build_messages,
                                        load_prompt)


@dataclass
class RetryReport:
    system: str
    workflow_id: str
    completed: bool = False
    stage_counts: dict = field(default_factory=dict)
    t_total_s: float = 0.0
    detail: str = ""


async def run_retry_langgraph(wf: WorkflowInput, port: LLMPort, prompts_dir: str,
                              entry: str, counter, max_attempts: int = 3,
                              initial_interval: float = 0.01) -> RetryReport:
    rep = RetryReport(system="langgraph", workflow_id=wf.workflow_id)
    policy = RetryPolicy(max_attempts=max_attempts, retry_on=Exception,
                         initial_interval=initial_interval, jitter=False)
    t0 = time.monotonic()
    result = await run_seq_langgraph(wf, port, prompts_dir, entry,
                                     retry_policy=policy)
    rep.t_total_s = time.monotonic() - t0
    rep.completed = result.status == "completed" and len(result.stages) == 3
    rep.stage_counts = dict(counter.counts)
    return rep


class RetryingStageExecutor(StageExecutor):
    """Executor MAF com re-execução interna (teto `max_attempts`) de falhas transientes."""

    def __init__(self, *a, max_attempts: int = 3, backoff_s: float = 0.01, **kw):
        super().__init__(*a, **kw)
        self._max_attempts = max_attempts
        self._backoff_s = backoff_s

    @handler
    async def run(self, message: object, ctx: WorkflowContext) -> None:
        carried = getattr(message, "analysis", None)
        if self._stage == "C" and carried:
            prior = ((ANALYSIS, carried), (REVIEW, getattr(message, "content")))
        else:
            prior = tuple((label, self._shared[label]) for label in self._prior_labels)
        messages = build_messages(load_prompt(self._prompts_dir, self._prompt_file),
                                  self._entry, prior)
        content: str | None = None
        last: Exception | None = None
        for attempt in range(1, self._max_attempts + 1):
            try:
                content, _ = await self._port.acomplete(
                    messages, self._stage, tuple(c for _, c in prior))
                last = None
                break
            except Exception as e:  # transiente: repõe dentro do teto
                last = e
                if attempt < self._max_attempts:
                    await asyncio.sleep(self._backoff_s * attempt)
        if last is not None or content is None:
            raise last or RuntimeError("retry exaurido")
        self._shared[self._publish_label] = content
        self._record.append(StageOutput(request_id=self._stage, step_id=self._stage,
                                        content=content))
        if self._stage == "C":
            await ctx.yield_output(OutC(content=content))
        elif self._stage == "B":
            await ctx.send_message(OutB(content=content,
                                        analysis=self._shared[ANALYSIS]))
        else:
            await ctx.send_message(OutA(content=content))


async def run_retry_maf(wf: WorkflowInput, port: LLMPort, prompts_dir: str,
                        entry: str, counter, max_attempts: int = 3) -> RetryReport:
    from agent_framework import WorkflowBuilder

    rep = RetryReport(system="maf", workflow_id=wf.workflow_id)
    shared: dict = {}
    record: list[StageOutput] = []

    def mk(id_suffix: str, prompt_file: str, stage: str, publish: str,
           prior: tuple[str, ...]) -> RetryingStageExecutor:
        return RetryingStageExecutor(id=f"{wf.workflow_id}-{id_suffix}", port=counter,
                                     prompts_dir=prompts_dir, prompt_file=prompt_file,
                                     stage=stage, publish_label=publish,
                                     prior_labels=prior, entry=entry,
                                     shared=shared, record=record,
                                     max_attempts=max_attempts)
    a = mk("a", "seq_A_analyst_v1.txt", "A", ANALYSIS, ())
    b = mk("b", "seq_B_reviewer_v1.txt", "B", REVIEW, (ANALYSIS,))
    c = mk("c", "seq_C_consolidator_v1.txt", "C", "final", (ANALYSIS, REVIEW))
    w = (WorkflowBuilder(start_executor=a, name=wf.workflow_id)
         .add_edge(a, b).add_edge(b, c).build())
    t0 = time.monotonic()
    await w.run("go")
    rep.t_total_s = time.monotonic() - t0
    rep.completed = len(record) == 3
    rep.stage_counts = dict(counter.counts)
    return rep


def build_retry_seq_graph(port: LLMPort, prompts_dir: str, max_attempts: int = 3):
    """Grafo seq com RetryPolicy — exposto p/ reuso no driver."""
    return build_seq_graph(port, prompts_dir, retry_policy=RetryPolicy(
        max_attempts=max_attempts, initial_interval=0.01, jitter=False))
