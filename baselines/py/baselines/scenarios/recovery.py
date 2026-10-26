"""Cenário recovery (REQ-907-004): falha 1× em C após B salvo + retomada nativa.

Aceite (padrão EXP4/880): A/B executam 1×; C completa na retomada; tempos medidos
com relógio monotônico. Mecanismos distintos por framework — LG retoma do
checkpointer (no grafo), MAF do checkpoint (superstep); comparável só como
"workflow completou + custo", nunca como mecanismo.
"""

from __future__ import annotations

import time
from dataclasses import dataclass, field

from langgraph.checkpoint.memory import InMemorySaver

from baselines.ports import LLMPort
from baselines.contracts import StageOutput, WorkflowInput
from baselines.systems.langgraph.seq import build_seq_graph
from baselines.systems.maf.seq import (MAF_SEQ_MSG_TYPES, build_seq_workflow)


@dataclass
class RecoveryReport:
    system: str
    workflow_id: str
    failed_first_run: bool = False
    completed_on_resume: bool = False
    stage_counts: dict = field(default_factory=dict)
    t_fail_s: float = 0.0
    t_recover_s: float = 0.0
    stages: tuple[StageOutput, ...] = ()
    detail: str = ""


async def run_recovery_langgraph(wf: WorkflowInput, port: LLMPort,
                                 prompts_dir: str, entry: str,
                                 counter) -> RecoveryReport:
    """`counter` deve envolver `port` (CallCounterPort) para o aceite."""
    app = build_seq_graph(port, prompts_dir, checkpointer=InMemorySaver())
    cfg = {"configurable": {"thread_id": wf.workflow_id}}
    rep = RecoveryReport(system="langgraph", workflow_id=wf.workflow_id)
    t0 = time.monotonic()
    try:
        await app.ainvoke({"entry": entry}, cfg)
        rep.detail = "1º run não falhou (porta sem falha?)"
        return rep
    except Exception:
        rep.failed_first_run = True
    rep.t_fail_s = time.monotonic() - t0
    t0 = time.monotonic()
    out = await app.ainvoke(None, cfg)
    rep.t_recover_s = time.monotonic() - t0
    rep.completed_on_resume = all(k in out for k in ("analysis", "review", "final"))
    rep.stage_counts = dict(counter.counts)
    return rep


async def _find_checkpoint_with_b_output(store, workflow_name: str, b_content: str):
    """Seleção causal (nunca índice/timestamp): checkpoint cujo estado contém OutB==b_content."""
    cps = await store.list_checkpoints(workflow_name=workflow_name)
    for cp in cps:
        full = await store.load(cp.checkpoint_id)
        for msgs in (full.messages or {}).values():
            for m in msgs or []:
                data = getattr(m, "data", None)
                if type(data).__name__ == "OutB" and getattr(data, "content", None) == b_content:
                    return full
    return None


async def run_recovery_maf(wf: WorkflowInput, port: LLMPort, prompts_dir: str,
                           entry: str, storage_dir: str, counter) -> RecoveryReport:
    """`counter` envolve `port`; `storage_dir` recebe o FileCheckpointStorage."""
    from agent_framework import FileCheckpointStorage

    store = FileCheckpointStorage(storage_dir,
                                  allowed_checkpoint_types=MAF_SEQ_MSG_TYPES)
    rep = RecoveryReport(system="maf", workflow_id=wf.workflow_id)
    record: list[StageOutput] = []
    shared: dict = {}
    w1 = build_seq_workflow(port, prompts_dir, entry, record, shared,
                            name=wf.workflow_id)
    t0 = time.monotonic()
    try:
        await w1.run("go", checkpoint_storage=store)
        rep.detail = "1º run não falhou (porta sem falha?)"
        return rep
    except Exception:
        rep.failed_first_run = True
    rep.t_fail_s = time.monotonic() - t0
    b_content = next(s.content for s in record if s.step_id == "B")
    chosen = await _find_checkpoint_with_b_output(store, wf.workflow_id, b_content)
    if chosen is None:
        rep.detail = "sem checkpoint com saída validada de B"
        return rep
    record2: list[StageOutput] = []
    shared2: dict = {}
    w2 = build_seq_workflow(port, prompts_dir, entry, record2, shared2,
                            name=wf.workflow_id)
    t0 = time.monotonic()
    r2 = await w2.run(checkpoint_id=chosen.checkpoint_id, checkpoint_storage=store)
    rep.t_recover_s = time.monotonic() - t0
    rep.completed_on_resume = bool(r2.get_outputs())
    rep.stage_counts = dict(counter.counts)
    rep.stages = tuple(record2)
    return rep
