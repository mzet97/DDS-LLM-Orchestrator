"""Adapter LangGraph `fork_join_v1` FULL: fan-out + barreira explícita (REQ-907-003).

A e B liberados pela entrada; `add_edge(["A", "B"], "C")` como barreira. Campos
distintos por ramo; C monta em ordem canônica. `limit_p=1` executa a variante
serial de controle (mesmo grafo, max_concurrency=1).
"""

from __future__ import annotations

from typing import TypedDict

from langgraph.graph import END, START, StateGraph
from langgraph.types import RetryPolicy

from baselines.ports import LLMPort
from baselines.contracts import StageOutput, WorkflowInput, WorkflowResult
from baselines.systems.assembly import CORRECTNESS, SECURITY, build_messages, load_prompt


class ForkState(TypedDict, total=False):
    entry: str
    branch_a: str
    branch_b: str
    final: str
    u_A: dict
    u_B: dict
    u_C: dict


def _branch(port: LLMPort, prompts_dir: str, prompt_file: str,
            stage: str, out_key: str):
    async def node(state: ForkState) -> dict:
        messages = build_messages(load_prompt(prompts_dir, prompt_file), state["entry"])
        content, usage = await port.acomplete(messages, stage, ())
        return {out_key: content, f"u_{stage}": usage}
    return node


def _consolidator(port: LLMPort, prompts_dir: str):
    async def node(state: ForkState) -> dict:
        prior = ((CORRECTNESS, state["branch_a"]), (SECURITY, state["branch_b"]))
        messages = build_messages(load_prompt(prompts_dir, "fork_C_consolidator_v1.txt"),
                                  state["entry"], prior)
        content, usage = await port.acomplete(
            messages, "C", tuple(c for _, c in prior))
        return {"final": content, "u_C": usage}
    return node


def build_forkjoin_graph(port: LLMPort, prompts_dir: str,
                         retry_policy: RetryPolicy | None = None):
    g: StateGraph = StateGraph(ForkState)
    kw = {"retry_policy": retry_policy} if retry_policy is not None else {}
    g.add_node("A", _branch(port, prompts_dir, "fork_A_correctness_v1.txt", "A", "branch_a"), **kw)
    g.add_node("B", _branch(port, prompts_dir, "fork_B_security_v1.txt", "B", "branch_b"), **kw)
    g.add_node("C", _consolidator(port, prompts_dir), **kw)
    g.add_edge(START, "A")
    g.add_edge(START, "B")
    g.add_edge(["A", "B"], "C")
    g.add_edge("C", END)
    return g.compile()


def _stages(out: dict) -> tuple[StageOutput, ...]:
    def so(step: str, key: str) -> StageOutput:
        u = out.get(f"u_{step}", {})
        return StageOutput(request_id=step, step_id=step, content=out[key],
                           tokens_prompt=int(u.get("prompt_tokens", 0)),
                           tokens_completion=int(u.get("completion_tokens", 0)))
    return (so("A", "branch_a"), so("B", "branch_b"), so("C", "final"))


async def run_forkjoin_langgraph(wf: WorkflowInput, port: LLMPort, prompts_dir: str,
                                 entry: str, limit_p: int = 2,
                                 retry_policy: RetryPolicy | None = None) -> WorkflowResult:
    app = build_forkjoin_graph(port, prompts_dir, retry_policy=retry_policy)
    out = await app.ainvoke(
        {"entry": entry},
        {"max_concurrency": limit_p, "configurable": {"thread_id": wf.workflow_id}})
    return WorkflowResult(workflow_id=wf.workflow_id, workload_id=wf.workload_id,
                          status="completed", stages=_stages(out))
