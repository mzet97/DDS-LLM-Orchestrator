"""Lado DDS (REQ-907-009): usa os bins existentes SEM modificá-los.

- Caminho primário por estágio: bin `submit-one` (crate client, `--features dds`) —
  task real reivindicada via DDS por agente(s) `agent --engine http`; devolve
  content + latency_ms + ttfc_ms + tokens. Encadeamento A→B→C dirigido AQUI
  (mesma regra de montagem dos baselines: assembly.build_messages).
- Caminho alternativo `DdsSyncClient`: HTTP `/api/v1/chat/completions/sync` do
  orchestrator — devolve T1–T6 (t_serialization/transport_send/agent_queue/
  inference/transport_return/deserialization) mas NÃO devolve content (contrato
  atual do handler) — serve para decomposição de overhead de estágio único.
"""

from __future__ import annotations

import asyncio
import json
import time
from dataclasses import dataclass

import httpx

from baselines.contracts import StageOutput, WorkflowInput, WorkflowResult
from baselines.systems.assembly import (ANALYSIS, CORRECTNESS, REVIEW, SECURITY,
                                        build_messages, load_prompt)


@dataclass
class DdsStageTiming:
    latency_ms: int = 0
    ttfc_ms: float = 0.0
    tokens_prompt: int = 0
    tokens_completion: int = 0


class DdsSubmitOne:
    """Porta LLMPort sobre o bin `submit-one` (subprocesso por estágio)."""

    def __init__(self, bin_path: str, domain: int, model: str,
                 timeout_ms: int = 120_000, max_tokens: int = 512,
                 temperature: float = 0.0):
        self.bin_path = bin_path
        self.domain = domain
        self.model = model
        self.timeout_ms = timeout_ms
        self.max_tokens = max_tokens
        self.temperature = temperature

    async def acomplete(self, messages: list[dict], stage: str,
                        expects: tuple[str, ...] = ()) -> tuple[str, dict]:
        t0 = time.monotonic()
        proc = await asyncio.create_subprocess_exec(
            self.bin_path, str(self.domain), self.model,
            json.dumps(messages, ensure_ascii=False),
            "--timeout-ms", str(self.timeout_ms),
            "--max-tokens", str(self.max_tokens),
            "--temperature", str(self.temperature),
            stdout=asyncio.subprocess.PIPE, stderr=asyncio.subprocess.PIPE)
        stdout, stderr = await proc.communicate()
        total_s = time.monotonic() - t0
        if proc.returncode != 0:
            raise RuntimeError(f"submit-one stage={stage} rc={proc.returncode}: "
                               f"{stderr.decode(errors='replace')[:300]}")
        try:
            out = json.loads(stdout.decode())
            content = out["content"]
            assert out.get("success") is True
        except (ValueError, KeyError, AssertionError) as e:
            raise RuntimeError(f"submit-one stage={stage}: saída inválida ({e}): "
                               f"{stdout.decode(errors='replace')[:200]}") from e
        usage = {"prompt_tokens": int(out.get("tokens_prompt", 0)),
                 "completion_tokens": int(out.get("tokens_completion", 0)),
                 "latency_ms": out.get("latency_ms", 0),
                 "ttfc_ms": out.get("ttfc_ms", 0),
                 "harness_overhead_s": total_s,
                 "task_id": out.get("task_id", "")}
        return content, usage


class DdsSyncClient:
    """Estágio único via orchestrator HTTP /sync — T1–T6 sem content."""

    def __init__(self, base_url: str, model: str, timeout_s: float = 180.0,
                 transport: httpx.AsyncBaseTransport | None = None):
        self.base_url = base_url.rstrip("/")
        self.model = model
        self.timeout_s = timeout_s
        self._transport = transport

    T_FIELDS = ("t_serialization_ns", "t_transport_send_ns", "t_agent_queue_ns",
                "t_inference_ns", "t_transport_return_ns", "t_deserialization_ns")

    async def stage_timings(self, messages: list[dict]) -> dict:
        """Devolve {status, latency_ms, t_*_ns} — para decomposição EXP1-style."""
        async with httpx.AsyncClient(timeout=self.timeout_s,
                                     transport=self._transport) as c:
            r = await c.post(f"{self.base_url}/api/v1/chat/completions/sync",
                             json={"model": self.model, "messages": messages})
            r.raise_for_status()
            body = r.json()
        out = {"status": body.get("status"),
               "latency_ms": body.get("latency_ms"),
               "assigned_agent": body.get("assigned_agent", "")}
        for f in self.T_FIELDS:
            out[f] = body.get(f, 0)
        return out


def _stage_out(stage: str, content: str, usage: dict) -> StageOutput:
    return StageOutput(request_id=stage, step_id=stage, content=content,
                       tokens_prompt=int(usage.get("prompt_tokens", 0)),
                       tokens_completion=int(usage.get("completion_tokens", 0)))


async def run_seq_dds(wf: WorkflowInput, dds: DdsSubmitOne, prompts_dir: str,
                      entry: str) -> WorkflowResult:
    """A→B→C com encadeamento dirigido pelo harness (padrão EXP0/880)."""
    ma = build_messages(load_prompt(prompts_dir, "seq_A_analyst_v1.txt"), entry)
    ca, ua = await dds.acomplete(ma, "A")
    mb = build_messages(load_prompt(prompts_dir, "seq_B_reviewer_v1.txt"), entry,
                        ((ANALYSIS, ca),))
    cb, ub = await dds.acomplete(mb, "B")
    mc = build_messages(load_prompt(prompts_dir, "seq_C_consolidator_v1.txt"), entry,
                        ((ANALYSIS, ca), (REVIEW, cb)))
    cc, uc = await dds.acomplete(mc, "C")
    return WorkflowResult(workflow_id=wf.workflow_id, workload_id=wf.workload_id,
                          status="completed",
                          stages=(_stage_out("A", ca, ua), _stage_out("B", cb, ub),
                                  _stage_out("C", cc, uc)))


async def run_concurrency_dds(wf: WorkflowInput, dds: DdsSubmitOne, prompts_dir: str,
                              entry: str, c: int) -> dict:
    """c workflows seq simultâneos: gather de submit-one → tasks disputadas por agentes.

    Devolve {completed, failed, makespan_s, per_workflow_s, errors} no formato do
    ConcurrencyReport dos baselines in-process.
    """
    t0 = time.monotonic()

    async def one(i: int):
        s0 = time.monotonic()
        wfi = WorkflowInput(case_id=wf.case_id, workflow_id=f"{wf.workflow_id}-{i}",
                            workload_id="seq_chain_v1")
        r = await run_seq_dds(wfi, dds, prompts_dir, entry)
        return r, time.monotonic() - s0

    results = await asyncio.gather(*[one(i) for i in range(c)], return_exceptions=True)
    completed = failed = 0
    per_wf: list[float] = []
    errors: list[str] = []
    for r in results:
        if isinstance(r, Exception):
            failed += 1
            errors.append(str(r)[:200])
            continue
        res, dt = r
        if res.status == "completed" and len(res.stages) == 3:
            completed += 1
            per_wf.append(dt)
        else:
            failed += 1
            errors.append(f"status={res.status}")
    return {"system": "dds", "concurrency": c, "completed": completed,
            "failed": failed, "makespan_s": time.monotonic() - t0,
            "per_workflow_s": per_wf, "errors": errors}


async def run_forkjoin_dds(wf: WorkflowInput, dds: DdsSubmitOne, prompts_dir: str,
                           entry: str, serial: bool = False) -> WorkflowResult:
    """Fork-join: A∥B (2 tasks → claim disputável entre agentes) + C canônico.

    `serial=True`: variante de controle fork_join_serial_v1 (cadeia, B sem consumir A).
    """
    ma = build_messages(load_prompt(prompts_dir, "fork_A_correctness_v1.txt"), entry)
    mb = build_messages(load_prompt(prompts_dir, "fork_B_security_v1.txt"), entry)
    if serial:
        ca, ua = await dds.acomplete(ma, "A")
        cb, ub = await dds.acomplete(mb, "B")
    else:
        (ca, ua), (cb, ub) = await asyncio.gather(dds.acomplete(ma, "A"),
                                                  dds.acomplete(mb, "B"))
    mc = build_messages(load_prompt(prompts_dir, "fork_C_consolidator_v1.txt"), entry,
                        ((CORRECTNESS, ca), (SECURITY, cb)))
    cc, uc = await dds.acomplete(mc, "C")
    return WorkflowResult(workflow_id=wf.workflow_id, workload_id=wf.workload_id,
                          status="completed",
                          stages=(_stage_out("A", ca, ua), _stage_out("B", cb, ub),
                                  _stage_out("C", cc, uc)))
