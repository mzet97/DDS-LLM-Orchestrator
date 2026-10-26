"""Stream (T-907-07 / REQ-907-007) + concorrência (T-907-08 / REQ-907-006)."""

from __future__ import annotations

import asyncio
import threading
from pathlib import Path

import pytest

from baselines.backends.deterministic_server import serve
from baselines.contracts import WorkflowInput
from baselines.ports import DeterministicPort
from baselines.providers.deterministic import DeterministicClient
from baselines.providers.llama import LlamaClient
from baselines.scenarios.concurrency import (run_concurrency_langgraph,
                                             run_concurrency_maf)
from baselines.scenarios.stream import (messages_for, stream_direct,
                                        stream_langgraph, stream_maf)

PROMPTS = str(Path(__file__).resolve().parents[1] / "baselines" / "prompts")
ENTRY = "O checkpoint incremental salva apenas páginas alteradas desde o último marco."


@pytest.fixture()
def backend():
    srv, state = serve(port=0, delay_ms=25, capacity=8)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield srv, state
    srv.shutdown()


def url(srv) -> str:
    return f"http://127.0.0.1:{srv.server_address[1]}"


def test_stream_direct_granularidade_token(backend):
    srv, _ = backend
    client = LlamaClient(url(srv), "deterministico")

    async def go():
        return await stream_direct(lambda m: client.astream(m),
                                   messages_for(PROMPTS, ENTRY))

    rep = asyncio.run(go())
    assert rep.granularity == "token"
    assert rep.ttfc_s is not None and rep.ttfc_s >= 0
    assert rep.text.startswith("[fixture")  # 2 metades SSE concatenadas (sem `bench`: stage=?)


def test_stream_langgraph_granularidade_superstep(backend):
    srv, _ = backend
    port = DeterministicPort(DeterministicClient(url(srv)))
    wf = WorkflowInput(case_id="caso-05", workflow_id="st-lg", workload_id="seq_chain_v1")
    rep = asyncio.run(stream_langgraph(wf, port, PROMPTS, ENTRY))
    assert rep.granularity == "superstep"
    assert rep.ttfc_s is not None and rep.text.startswith("[fixture stage=C")


def test_stream_maf_granularidade_superstep(backend):
    srv, _ = backend
    port = DeterministicPort(DeterministicClient(url(srv)))
    wf = WorkflowInput(case_id="caso-05", workflow_id="st-maf", workload_id="seq_chain_v1")
    rep = asyncio.run(stream_maf(wf, port, PROMPTS, ENTRY))
    assert rep.granularity == "superstep"
    assert rep.ttfc_s is not None and rep.text.startswith("[fixture stage=C")


def test_langgraph_concorrencia_c4_isolado(backend):
    srv, _ = backend
    port = DeterministicPort(DeterministicClient(url(srv)))
    wf = WorkflowInput(case_id="caso-01", workflow_id="cc-lg", workload_id="seq_chain_v1")
    rep = asyncio.run(run_concurrency_langgraph(wf, port, PROMPTS, ENTRY, c=4))
    assert rep.completed == 4 and rep.failed == 0, rep.errors
    assert len(rep.per_workflow_s) == 4
    # Paralelismo real: 4 workflows seq (3 estágios × 25 ms) não levam 4× serial.
    assert rep.makespan_s < 4 * 3 * 0.025 * 2, rep.makespan_s


def test_maf_concorrencia_c4_isolado(backend):
    srv, _ = backend
    port = DeterministicPort(DeterministicClient(url(srv)))
    wf = WorkflowInput(case_id="caso-01", workflow_id="cc-maf", workload_id="seq_chain_v1")
    rep = asyncio.run(run_concurrency_maf(wf, port, PROMPTS, ENTRY, c=4))
    assert rep.completed == 4 and rep.failed == 0, rep.errors
    assert rep.makespan_s < 4 * 3 * 0.025 * 2, rep.makespan_s
