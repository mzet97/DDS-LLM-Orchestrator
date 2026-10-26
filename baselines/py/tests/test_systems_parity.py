"""Paridade full-framework (T-907-04/05 / REQ-907-002/003): direct × LangGraph × MAF.

Backend determinístico real (HTTP localhost): conteúdos byte-idênticos entre os
3 sistemas em seq e fork-join; fork-join paralelo com sobreposição real (controle
da Fase 1 do EXP0) e serial sem sobreposição; hashes de request idênticos por estágio.
"""

from __future__ import annotations

import asyncio
import threading

import pytest

from baselines.backends.deterministic_server import serve
from baselines.contracts import WorkflowInput
from baselines.ports import DeterministicPort
from baselines.systems.direct import control
from baselines.systems.langgraph.forkjoin import run_forkjoin_langgraph
from baselines.systems.langgraph.seq import run_seq_langgraph
from baselines.systems.maf.forkjoin import run_forkjoin_maf
from baselines.systems.maf.seq import run_seq_maf

PROMPTS = str(__import__("pathlib").Path(__file__).resolve().parents[1]
              / "baselines" / "prompts")
ENTRY = "O retry exponencial para após 5 tentativas e registra a causa."


@pytest.fixture()
def backend():
    srv, state = serve(port=0, delay_ms=25, capacity=8)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield srv, state
    srv.shutdown()


def url(srv) -> str:
    return f"http://127.0.0.1:{srv.server_address[1]}"


def stage_times(state, stage: str) -> list[tuple[int, int]]:
    return [(r["arrival_ns"], r["finished_ns"]) for r in state.requests
            if r["stage"] == stage]


def contents(result) -> dict[str, str]:
    return {s.step_id: s.content for s in result.stages}


def test_seq_parity_3_sistemas_byte_idêntico(backend):
    srv, state = backend
    from baselines.providers.deterministic import DeterministicClient
    port = DeterministicPort(DeterministicClient(url(srv)))
    n0 = len(state.requests)

    wf_d = WorkflowInput(case_id="caso-04", workflow_id="seq-d", workload_id="seq_chain_v1")
    r_direct = control.run_seq_direct(wf_d, url(srv), PROMPTS, ENTRY)
    wf_l = WorkflowInput(case_id="caso-04", workflow_id="seq-lg", workload_id="seq_chain_v1")
    r_lg = asyncio.run(run_seq_langgraph(wf_l, port, PROMPTS, ENTRY))
    wf_m = WorkflowInput(case_id="caso-04", workflow_id="seq-maf", workload_id="seq_chain_v1")
    r_maf = asyncio.run(run_seq_maf(wf_m, port, PROMPTS, ENTRY))

    assert contents(r_direct) == contents(r_lg) == contents(r_maf)
    assert all(c.startswith("[fixture stage=") for c in contents(r_direct).values())
    assert len(state.requests) - n0 == 9  # 3 estágios × 3 sistemas


def test_forkjoin_parity_3_sistemas_e_sobreposição(backend):
    srv, state = backend
    from baselines.providers.deterministic import DeterministicClient
    port = DeterministicPort(DeterministicClient(url(srv)))
    n0 = len(state.requests)

    wf_d = WorkflowInput(case_id="caso-02", workflow_id="fj-d", workload_id="fork_join_v1")
    r_direct = asyncio.run(control.run_forkjoin_direct(wf_d, url(srv), PROMPTS, ENTRY))
    wf_l = WorkflowInput(case_id="caso-02", workflow_id="fj-lg", workload_id="fork_join_v1")
    r_lg = asyncio.run(run_forkjoin_langgraph(wf_l, port, PROMPTS, ENTRY, limit_p=2))
    wf_m = WorkflowInput(case_id="caso-02", workflow_id="fj-maf", workload_id="fork_join_v1")
    r_maf = asyncio.run(run_forkjoin_maf(wf_m, port, PROMPTS, ENTRY, serial=False))

    assert contents(r_direct) == contents(r_lg) == contents(r_maf)
    # Sobreposição real nos paralelos: B chega antes de A terminar (delay 25 ms).
    a_arr, a_fin = stage_times(state, "A")[0]
    b_arr, _ = stage_times(state, "B")[0]
    assert b_arr < a_fin, "fork-join paralelo deve sobrepor A e B"
    assert a_arr > 0
    assert len(state.requests) - n0 == 9


def test_serial_sem_sobreposição_e_mesmo_conteúdo(backend):
    srv, state = backend
    from baselines.providers.deterministic import DeterministicClient
    port = DeterministicPort(DeterministicClient(url(srv)))
    n0 = len(state.requests)

    wf_l = WorkflowInput(case_id="caso-02", workflow_id="fjs-lg", workload_id="fork_join_serial_v1")
    r_lg = asyncio.run(run_forkjoin_langgraph(wf_l, port, PROMPTS, ENTRY, limit_p=1))
    wf_m = WorkflowInput(case_id="caso-02", workflow_id="fjs-maf", workload_id="fork_join_serial_v1")
    r_maf = asyncio.run(run_forkjoin_maf(wf_m, port, PROMPTS, ENTRY, serial=True))

    assert contents(r_lg) == contents(r_maf)
    (_, a_fin), (b_arr, _) = stage_times(state, "A")[0], stage_times(state, "B")[0]
    assert b_arr >= a_fin, "serial não deve sobrepor A e B"
    assert len(state.requests) - n0 == 6
