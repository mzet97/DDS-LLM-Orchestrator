"""Adapter LangGraph `seq_chain_v1` FULL: StateGraph START→A→B→C→END (REQ-907-002).

Nós assíncronos; cada um monta a entrada autorizada, faz UMA chamada via porta e
retorna apenas sua atualização de estado. Sem loop ReAct. `checkpointer` habilita
a retomada nativa por `thread_id` (REQ-907-004); `retry_policy` é a nativa do
LangGraph (REQ-907-005).
"""

from __future__ import annotations

from typing import TypedDict

from langgraph.graph import END, START, StateGraph
from langgraph.types import RetryPolicy

from baselines.ports import LLMPort
from baselines.contracts import StageOutput, WorkflowInput, WorkflowResult
from baselines.systems.assembly import ANALYSIS, REVIEW, build_messages, load_prompt

LABELS = {"analysis": ANALYSIS, "review": REVIEW}


class SeqState(TypedDict, total=False):
    entry: str
    analysis: str
    review: str
    final: str
    u_A: dict
    u_B: dict
    u_C: dict


def _make_node(port: LLMPort, prompts_dir: str, prompt_file: str,
               stage: str, out_key: str, needs: tuple[str, ...]):
    async def node(state: SeqState) -> dict:
        prior = tuple((LABELS[k], state[k]) for k in needs)
        messages = build_messages(load_prompt(prompts_dir, prompt_file),
                                  state["entry"], prior)
        content, usage = await port.acomplete(messages, stage,
                                              tuple(c for _, c in prior))
        return {out_key: content, f"u_{stage}": usage}
    return node


def build_seq_graph(port: LLMPort, prompts_dir: str, checkpointer=None,
                    retry_policy: RetryPolicy | None = None):
    g: StateGraph = StateGraph(SeqState)
    kw = {"retry_policy": retry_policy} if retry_policy is not None else {}
    g.add_node("A", _make_node(port, prompts_dir, "seq_A_analyst_v1.txt", "A", "analysis", ()), **kw)
    g.add_node("B", _make_node(port, prompts_dir, "seq_B_reviewer_v1.txt", "B", "review", ("analysis",)), **kw)
    g.add_node("C", _make_node(port, prompts_dir, "seq_C_consolidator_v1.txt", "C", "final", ("analysis", "review")), **kw)
    g.add_edge(START, "A")
    g.add_edge("A", "B")
    g.add_edge("B", "C")
    g.add_edge("C", END)
    return g.compile(checkpointer=checkpointer)


def _stages(out: dict) -> tuple[StageOutput, ...]:
    def so(step: str, key: str) -> StageOutput:
        u = out.get(f"u_{step}", {})
        return StageOutput(request_id=step, step_id=step, content=out[key],
                           tokens_prompt=int(u.get("prompt_tokens", 0)),
                           tokens_completion=int(u.get("completion_tokens", 0)))
    return (so("A", "analysis"), so("B", "review"), so("C", "final"))


async def run_seq_langgraph(wf: WorkflowInput, port: LLMPort, prompts_dir: str,
                            entry: str, checkpointer=None,
                            retry_policy: RetryPolicy | None = None) -> WorkflowResult:
    app = build_seq_graph(port, prompts_dir, checkpointer=checkpointer,
                          retry_policy=retry_policy)
    out = await app.ainvoke({"entry": entry},
                            {"configurable": {"thread_id": wf.workflow_id}})
    return WorkflowResult(workflow_id=wf.workflow_id, workload_id=wf.workload_id,
                          status="completed", stages=_stages(out))
