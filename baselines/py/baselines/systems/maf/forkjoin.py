"""Adapter MAF `fork_join_v1` FULL: fan-out A/B + barreira fan-in → C (REQ-907-003).

Topologia: Source → fan-out [A, B] → fan-in [A, B] → C. Source é controle
determinístico (zero chamadas de inferência). C lê A/B em ordem canônica do
estado indexado, independente da ordem de chegada. `serial=True` executa a
variante de controle `fork_join_serial_v1` (cadeia s→a→b→c, B não consome A).
"""

from __future__ import annotations

from dataclasses import dataclass

from agent_framework import Executor, WorkflowBuilder, WorkflowContext, handler

from baselines.ports import LLMPort
from baselines.contracts import StageOutput, WorkflowInput, WorkflowResult
from baselines.systems.assembly import CORRECTNESS, SECURITY, build_messages, load_prompt


@dataclass
class Go:
    text: str


@dataclass
class BranchA:
    content: str


@dataclass
class BranchB:
    content: str


@dataclass
class Consolidated:
    content: str


class Source(Executor):
    @handler
    async def run(self, message: object, ctx: WorkflowContext) -> None:
        await ctx.send_message(Go(text="go"))


class BranchExecutor(Executor):
    def __init__(self, id: str, port: LLMPort, prompts_dir: str,
                 prompt_file: str, stage: str, publish_label: str, entry: str,
                 shared: dict, record: list, out_cls: type):
        super().__init__(id=id)
        self._port = port
        self._prompts_dir = prompts_dir
        self._prompt_file = prompt_file
        self._stage = stage
        self._publish_label = publish_label
        self._entry = entry
        self._shared = shared
        self._record = record
        self._out_cls = out_cls

    @handler
    async def run(self, message: object, ctx: WorkflowContext) -> None:
        # Mensagem ignorada de propósito: ramos recebem só ENTRY (sem A_to_B).
        # `object` (em vez de `Go`) permite encadear a→b no modo serial.
        messages = build_messages(load_prompt(self._prompts_dir, self._prompt_file),
                                  self._entry)
        content, _ = await self._port.acomplete(messages, self._stage, ())
        self._shared[self._publish_label] = content
        self._record.append(StageOutput(request_id=self._stage, step_id=self._stage,
                                        content=content))
        await ctx.send_message(self._out_cls(content=content))


class Consolidator(Executor):
    def __init__(self, id: str, port: LLMPort, prompts_dir: str,
                 entry: str, shared: dict, record: list):
        super().__init__(id=id)
        self._port = port
        self._prompts_dir = prompts_dir
        self._entry = entry
        self._shared = shared
        self._record = record

    @handler
    async def run(self, message: object, ctx: WorkflowContext) -> None:
        prior = ((CORRECTNESS, self._shared[CORRECTNESS]),
                 (SECURITY, self._shared[SECURITY]))
        messages = build_messages(
            load_prompt(self._prompts_dir, "fork_C_consolidator_v1.txt"),
            self._entry, prior)
        content, _ = await self._port.acomplete(messages, "C", tuple(c for _, c in prior))
        self._record.append(StageOutput(request_id="C", step_id="C", content=content))
        await ctx.yield_output(Consolidated(content=content))


def build_forkjoin_workflow(port: LLMPort, prompts_dir: str, entry: str,
                            record: list, shared: dict, serial: bool = False,
                            name: str = "maf-forkjoin") -> object:
    s = Source(id=f"{name}-s")
    a = BranchExecutor(id=f"{name}-a", port=port, prompts_dir=prompts_dir,
                       prompt_file="fork_A_correctness_v1.txt", stage="A",
                       publish_label=CORRECTNESS, entry=entry, shared=shared,
                       record=record, out_cls=BranchA)
    b = BranchExecutor(id=f"{name}-b", port=port, prompts_dir=prompts_dir,
                       prompt_file="fork_B_security_v1.txt", stage="B",
                       publish_label=SECURITY, entry=entry, shared=shared,
                       record=record, out_cls=BranchB)
    c = Consolidator(id=f"{name}-c", port=port, prompts_dir=prompts_dir,
                     entry=entry, shared=shared, record=record)
    if serial:
        return (WorkflowBuilder(start_executor=s, name=name)
                .add_edge(s, a).add_edge(a, b).add_edge(b, c).build())
    return (WorkflowBuilder(start_executor=s, name=name)
            .add_fan_out_edges(s, [a, b])
            .add_fan_in_edges([a, b], c)
            .build())


async def run_forkjoin_maf(wf: WorkflowInput, port: LLMPort, prompts_dir: str,
                           entry: str, serial: bool = False) -> WorkflowResult:
    record: list[StageOutput] = []
    shared: dict = {}
    workflow = build_forkjoin_workflow(port, prompts_dir, entry, record, shared,
                                       serial=serial)
    await workflow.run("go")
    assert CORRECTNESS in shared and SECURITY in shared
    return WorkflowResult(workflow_id=wf.workflow_id, workload_id=wf.workload_id,
                          status="completed", stages=tuple(record))
