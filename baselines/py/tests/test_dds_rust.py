"""DDS side (T-907-09 / REQ-907-009): parsing do submit-one e encadeamento.

Sem DDS real nos testes unitários: um "submit-one falso" (script) devolve JSON
canônico; o teste prova parsing, encadeamento A→B→C e fork-join paralelo/serial.
O smoke REAL (bin + agentes + llama) roda só com BENCH_SUBMIT_ONE+DDS_DOMAIN
(padrão EXP0: skip limpo sem ambiente).
"""

from __future__ import annotations

import asyncio
import json
import os
import stat
import tempfile
from pathlib import Path

import pytest

from baselines.contracts import WorkflowInput
from baselines.systems.dds_rust import (DdsSubmitOne, DdsSyncClient,
                                        run_forkjoin_dds, run_seq_dds)

PROMPTS = str(Path(__file__).resolve().parents[1] / "baselines" / "prompts")
ENTRY = "O cache expira em 60 segundos e deve ser invalidado por escrita."

FAKE = """#!/usr/bin/env python3
import json, sys
args = sys.argv[1:]
domain, model, messages = args[0], args[1], json.loads(args[2])
# replica o formato canônico do submit-one (crates/client/src/bin/submit_one.rs)
print(json.dumps({"task_id": f"fake-{abs(hash(messages[0]['content']))%9999}",
                  "content": f"[fake {model} n={len(messages)}]",
                  "success": True, "latency_ms": 12, "ttfc_ms": 5.5,
                  "tokens_prompt": 10, "tokens_completion": 5}))
"""


@pytest.fixture()
def fake_submit_one():
    with tempfile.TemporaryDirectory() as d:
        p = Path(d) / "submit-one"
        p.write_text(FAKE)
        p.chmod(p.stat().st_mode | stat.S_IEXEC)
        yield str(p)


def test_seq_dds_encadeia_3_estagios(fake_submit_one):
    dds = DdsSubmitOne(fake_submit_one, domain=79, model="fake-model")
    wf = WorkflowInput(case_id="caso-01", workflow_id="dds-seq", workload_id="seq_chain_v1")
    r = asyncio.run(run_seq_dds(wf, dds, PROMPTS, ENTRY))
    assert r.status == "completed"
    a, b, c = r.stages
    assert a.step_id == "A" and b.step_id == "B" and c.step_id == "C"
    assert b.content == f"[fake fake-model n=3]"      # system+entry+analysis
    assert c.content == f"[fake fake-model n=4]"      # system+entry+analysis+review
    assert a.tokens_completion == 5 and c.tokens_prompt == 10


def test_forkjoin_dds_paralelo_e_serial(fake_submit_one):
    dds = DdsSubmitOne(fake_submit_one, domain=79, model="fake-model")
    wf = WorkflowInput(case_id="caso-02", workflow_id="dds-fj", workload_id="fork_join_v1")
    r = asyncio.run(run_forkjoin_dds(wf, dds, PROMPTS, ENTRY, serial=False))
    assert [s.step_id for s in r.stages] == ["A", "B", "C"]
    assert r.stages[2].content == "[fake fake-model n=4]"
    r2 = asyncio.run(run_forkjoin_dds(wf, dds, PROMPTS, ENTRY, serial=True))
    assert [s.step_id for s in r2.stages] == ["A", "B", "C"]


def test_erro_tipado_quando_submit_one_falha(tmp_path):
    p = tmp_path / "submit-one-fail"
    p.write_text("#!/usr/bin/env sh\nexit 3\n")
    p.chmod(p.stat().st_mode | stat.S_IEXEC)
    dds = DdsSubmitOne(str(p), domain=79, model="m")
    with pytest.raises(RuntimeError, match="rc=3"):
        asyncio.run(dds.acomplete([{"role": "user", "content": "x"}], "A"))


def test_sync_client_t1_a_t6_com_mock():
    body = {"task_id": "t1", "status": "completed", "latency_ms": 42,
            "assigned_agent": "agent-01", "t_serialization_ns": 1,
            "t_transport_send_ns": 2, "t_agent_queue_ns": 3, "t_inference_ns": 4,
            "t_transport_return_ns": 5, "t_deserialization_ns": 6}

    import httpx

    def handler(request: httpx.Request) -> httpx.Response:
        payload = json.loads(request.content)
        assert payload["model"] == "m"
        return httpx.Response(200, json=body)

    client = DdsSyncClient("http://mock", "m", transport=httpx.MockTransport(handler))
    out = asyncio.run(client.stage_timings([{"role": "user", "content": "x"}]))
    assert out["status"] == "completed" and out["t_inference_ns"] == 4
    assert out["assigned_agent"] == "agent-01"


@pytest.mark.skipif(not os.environ.get("BENCH_SUBMIT_ONE") or not os.environ.get("DDS_DOMAIN"),
                    reason="smoke real exige bins DDS + agentes no domínio (padrão EXP0)")
def test_smoke_dds_real_seq():
    dds = DdsSubmitOne(os.environ["BENCH_SUBMIT_ONE"], domain=int(os.environ["DDS_DOMAIN"]),
                       model=os.environ.get("DDS_MODEL", "gemma-4-12b"))
    wf = WorkflowInput(case_id="caso-01", workflow_id="smoke-dds", workload_id="seq_chain_v1")
    r = asyncio.run(run_seq_dds(wf, dds, PROMPTS, ENTRY))
    assert r.status == "completed" and all(len(s.content) > 0 for s in r.stages)
