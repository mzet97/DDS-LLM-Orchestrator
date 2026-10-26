"""Recovery + retry (T-907-06 / REQ-907-004/005): aceites por contagem de chamadas."""

from __future__ import annotations

import asyncio
import tempfile
import threading
from pathlib import Path

import pytest

from baselines.backends.deterministic_server import serve
from baselines.contracts import WorkflowInput
from baselines.ports import CallCounterPort, DeterministicPort, FaultyPort
from baselines.providers.deterministic import DeterministicClient
from baselines.scenarios.recovery import run_recovery_langgraph, run_recovery_maf
from baselines.scenarios.retry import run_retry_langgraph, run_retry_maf

PROMPTS = str(Path(__file__).resolve().parents[1] / "baselines" / "prompts")
ENTRY = "O retry exponencial para após 5 tentativas e registra a causa."


@pytest.fixture()
def backend():
    srv, state = serve(port=0, delay_ms=2, capacity=8)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield srv, state
    srv.shutdown()


def url(srv) -> str:
    return f"http://127.0.0.1:{srv.server_address[1]}"


def test_langgraph_recovery_fail_once_resume(backend):
    srv, _ = backend
    # Contador por DENTRO do FaultyPort = conta só chamadas reais ao backend
    # (a tentativa com falha aborta antes — semântica do aceite EXP4/880).
    cc = CallCounterPort(DeterministicPort(DeterministicClient(url(srv))))
    port = FaultyPort(cc, fail_on_stage="C", times=1)
    wf = WorkflowInput(case_id="caso-04", workflow_id="rec-lg", workload_id="seq_chain_v1")
    rep = asyncio.run(run_recovery_langgraph(wf, port, PROMPTS, ENTRY, cc))
    assert rep.failed_first_run, rep.detail
    assert rep.completed_on_resume
    # A/B sem repetição; C só executa na retomada (falha aborta antes da chamada)
    assert rep.stage_counts == {"A": 1, "B": 1, "C": 1}, rep.stage_counts
    assert rep.t_fail_s > 0 and rep.t_recover_s > 0


def test_maf_recovery_fail_once_resume(backend):
    srv, _ = backend
    cc = CallCounterPort(DeterministicPort(DeterministicClient(url(srv))))
    port = FaultyPort(cc, fail_on_stage="C", times=1)
    wf = WorkflowInput(case_id="caso-04", workflow_id="rec-maf", workload_id="seq_chain_v1")
    with tempfile.TemporaryDirectory() as d:
        rep = asyncio.run(run_recovery_maf(wf, port, PROMPTS, ENTRY, d, cc))
    assert rep.failed_first_run, rep.detail
    assert rep.completed_on_resume, rep.detail
    assert rep.stage_counts == {"A": 1, "B": 1, "C": 1}, rep.stage_counts
    assert [s.step_id for s in rep.stages] == ["C"]


def test_langgraph_retry_within_ceiling(backend):
    srv, _ = backend
    inner = DeterministicPort(DeterministicClient(url(srv)))
    counter = CallCounterPort(FaultyPort(inner, fail_on_stage="B", times=1))
    wf = WorkflowInput(case_id="caso-04", workflow_id="rty-lg", workload_id="seq_chain_v1")
    rep = asyncio.run(run_retry_langgraph(wf, counter, PROMPTS, ENTRY, counter))
    assert rep.completed
    # B: 1 tentativa que falhou (antes do backend) + 1 que passou
    assert rep.stage_counts == {"A": 1, "B": 2, "C": 1}, rep.stage_counts


def test_maf_retry_within_ceiling(backend):
    srv, _ = backend
    inner = DeterministicPort(DeterministicClient(url(srv)))
    counter = CallCounterPort(FaultyPort(inner, fail_on_stage="B", times=1))
    wf = WorkflowInput(case_id="caso-04", workflow_id="rty-maf", workload_id="seq_chain_v1")
    rep = asyncio.run(run_retry_maf(wf, counter, PROMPTS, ENTRY, counter))
    assert rep.completed
    assert rep.stage_counts == {"A": 1, "B": 2, "C": 1}, rep.stage_counts
