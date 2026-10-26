"""Adapter MAF `seq_chain_v1` FULL: WorkflowBuilder + Executor + arestas (REQ-907-002).

Cada executor monta a entrada autorizada (assembly.build_messages), faz UMA chamada
de inferência pela porta e devolve resultado tipado. Transições pertencem às arestas.
Os dataclasses de mensagem são o contrato do checkpoint nativo do MAF
(`allowed_checkpoint_types` referencia os caminhos `baselines.systems.maf.seq:OutX`).
"""

from __future__ import annotations

from dataclasses import dataclass

from agent_framework import Executor, WorkflowBuilder, WorkflowContext, handler

from baselines.ports import LLMPort
from baselines.contracts import StageOutput, WorkflowInput, WorkflowResult
from baselines.systems.assembly import ANALYSIS, REVIEW, build_messages, load_prompt

# Caminhos de tipo aceitos pelo checkpoint storage na retomada (padrão EXP4/880).
MAF_SEQ_MSG_TYPES = ["baselines.systems.maf.seq:OutA",
                     "baselines.systems.maf.seq:OutB",
                     "baselines.systems.maf.seq:OutC"]


@dataclass
class OutA:
    content: str


@dataclass
class OutB:
    content: str
    analysis: str = ""


@dataclass
class OutC:
    content: str


class StageExecutor(Executor):
    """Um estágio lógico: publica seu conteúdo sob `publish_label`."""

    def __init__(self, id: str, port: LLMPort, prompts_dir: str,
                 prompt_file: str, stage: str, publish_label: str,
                 prior_labels: tuple[str, ...], entry: str,
                 shared: dict, record: list):
        super().__init__(id=id)
        self._port = port
        self._prompts_dir = prompts_dir
        self._prompt_file = prompt_file
        self._stage = stage
        self._publish_label = publish_label
        self._prior_labels = prior_labels
        self._entry = entry
        self._shared = shared
        self._record = record

    @handler
    async def run(self, message: object, ctx: WorkflowContext) -> None:
        carried = getattr(message, "analysis", None)
        if self._stage == "C" and carried:
            prior = ((ANALYSIS, carried), (REVIEW, getattr(message, "content")))
        else:
            prior = tuple((label, self._shared[label]) for label in self._prior_labels)
        messages = build_messages(load_prompt(self._prompts_dir, self._prompt_file),
                                  self._entry, prior)
        content, _ = await self._port.acomplete(messages, self._stage,
                                                tuple(c for _, c in prior))
        self._shared[self._publish_label] = content
        self._record.append(StageOutput(request_id=self._stage, step_id=self._stage,
                                        content=content))
        if self._stage == "C":
            await ctx.yield_output(OutC(content=content))
        elif self._stage == "B":
            # Proveniência replayável: a mensagem carrega A para a retomada.
            await ctx.send_message(OutB(content=content,
                                        analysis=self._shared[ANALYSIS]))
        else:
            await ctx.send_message(OutA(content=content))


def build_seq_workflow(port: LLMPort, prompts_dir: str, entry: str,
                       record: list, shared: dict, name: str = "maf-seq") -> object:
    a = StageExecutor(id=f"{name}-a", port=port, prompts_dir=prompts_dir,
                      prompt_file="seq_A_analyst_v1.txt", stage="A",
                      publish_label=ANALYSIS, prior_labels=(), entry=entry,
                      shared=shared, record=record)
    b = StageExecutor(id=f"{name}-b", port=port, prompts_dir=prompts_dir,
                      prompt_file="seq_B_reviewer_v1.txt", stage="B",
                      publish_label=REVIEW, prior_labels=(ANALYSIS,), entry=entry,
                      shared=shared, record=record)
    c = StageExecutor(id=f"{name}-c", port=port, prompts_dir=prompts_dir,
                      prompt_file="seq_C_consolidator_v1.txt", stage="C",
                      publish_label="final", prior_labels=(ANALYSIS, REVIEW),
                      entry=entry, shared=shared, record=record)
    return (WorkflowBuilder(start_executor=a, name=name).add_edge(a, b).add_edge(b, c)
            .build())


async def run_seq_maf(wf: WorkflowInput, port: LLMPort, prompts_dir: str,
                      entry: str) -> WorkflowResult:
    record: list[StageOutput] = []
    shared: dict = {}
    workflow = build_seq_workflow(port, prompts_dir, entry, record, shared)
    await workflow.run("go")
    return WorkflowResult(workflow_id=wf.workflow_id, workload_id=wf.workload_id,
                          status="completed", stages=tuple(record))
