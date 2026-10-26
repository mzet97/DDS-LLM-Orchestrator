"""Cenário stream (REQ-907-007): TTFC do evento NATIVO de cada sistema — nunca simulado.

Granularidade honesta por sistema:
- `direct`/DDS HTTP: token-level do server (SSE do llama-server; TaskOutput chunks no DDS).
- LangGraph: `astream` expõe atualização de ESTADO por nó (superstep) — não há
  stream de token no caminho nó→porta não-streaming. TTFC = 1º update de nó.
- MAF: `run(stream=True)` expõe `WorkflowEvent` por executor — mesma granularidade
  de superstep. TTFC = 1º evento.
O REPORT marca PARCIAL onde o framework não expõe token-level (evidência no teste).
"""

from __future__ import annotations

import time
from dataclasses import dataclass

from baselines.ports import LLMPort
from baselines.contracts import WorkflowInput
from baselines.systems.langgraph.seq import build_seq_graph
from baselines.systems.maf.seq import build_seq_workflow
from baselines.systems.assembly import build_messages, load_prompt


@dataclass
class StreamReport:
    system: str
    workflow_id: str
    granularity: str = ""          # "token" | "superstep"
    ttfc_s: float | None = None    # time-to-first-chunk/evento
    total_s: float = 0.0
    text: str = ""


async def stream_direct(llama_port_stream, messages: list[dict]) -> StreamReport:
    """`llama_port_stream`: callable async -> aiter de (delta, ts). Granularidade token."""
    t0 = time.monotonic()
    ttfc: float | None = None
    parts: list[str] = []
    async for delta, ts in llama_port_stream(messages):
        if ttfc is None:
            ttfc = ts
        parts.append(delta)
    return StreamReport(system="direct", workflow_id="stream-direct",
                        granularity="token", ttfc_s=ttfc,
                        total_s=time.monotonic() - t0, text="".join(parts))


async def stream_langgraph(wf: WorkflowInput, port: LLMPort, prompts_dir: str,
                           entry: str) -> StreamReport:
    app = build_seq_graph(port, prompts_dir)
    t0 = time.monotonic()
    ttfc: float | None = None
    last: dict = {}
    async for chunk in app.astream({"entry": entry},
                                   {"configurable": {"thread_id": wf.workflow_id}},
                                   stream_mode="updates"):
        if ttfc is None:
            ttfc = time.monotonic() - t0
        last.update(chunk)
    rep = StreamReport(system="langgraph", workflow_id=wf.workflow_id,
                       granularity="superstep", ttfc_s=ttfc,
                       total_s=time.monotonic() - t0)
    final = last.get("C", {})
    rep.text = final.get("final", "")
    return rep


async def stream_maf(wf: WorkflowInput, port: LLMPort, prompts_dir: str,
                     entry: str) -> StreamReport:
    shared: dict = {}
    record: list = []
    w = build_seq_workflow(port, prompts_dir, entry, record, shared,
                           name=wf.workflow_id)
    t0 = time.monotonic()
    ttfc: float | None = None
    async for event in await w.run("go", stream=True):
        if ttfc is None:
            ttfc = time.monotonic() - t0
    rep = StreamReport(system="maf", workflow_id=wf.workflow_id,
                       granularity="superstep", ttfc_s=ttfc,
                       total_s=time.monotonic() - t0)
    rep.text = shared.get("final", "")
    return rep


def messages_for(prompts_dir: str, entry: str) -> list[dict]:
    return build_messages(load_prompt(prompts_dir, "seq_A_analyst_v1.txt"), entry)
