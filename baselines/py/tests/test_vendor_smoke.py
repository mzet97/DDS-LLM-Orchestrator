"""Fumaça do vendor (T-907-02 / REQ-907-001): pacote autossuficiente e fidelidade ao freeze 860."""

from __future__ import annotations

import hashlib
import json
from pathlib import Path

from baselines import contracts
from baselines.systems import assembly
from baselines.providers.deterministic import DeterministicClient
from baselines.systems.direct import control

PROMPTS_DIR = Path(__file__).resolve().parents[1] / "baselines" / "prompts"

# 16 primeiros hex do SHA-256 — specs/860-backends/manifest-freeze.md
FREEZE_860 = {
    "seq_A_analyst_v1.txt": "52bc20d062f49b18",
    "seq_B_reviewer_v1.txt": "6427c642776f0d82",
    "seq_C_consolidator_v1.txt": "021b0e28c85452ec",
    "fork_A_correctness_v1.txt": "99a1377689aaab48",
    "fork_B_security_v1.txt": "a9aeac0c0c22a6e8",
    "fork_C_consolidator_v1.txt": "999f1fe40283de4b",
}


def test_prompts_conferem_com_freeze_860():
    for nome, hash16 in FREEZE_860.items():
        data = (PROMPTS_DIR / nome).read_bytes()
        assert hashlib.sha256(data).hexdigest()[:16] == hash16, nome


def test_contratos_schema_e_workloads():
    assert contracts.SCHEMA_VERSION == 1
    assert contracts.WORKLOAD_SEQ_CHAIN == "seq_chain_v1"
    assert contracts.WORKLOAD_FORK_JOIN == "fork_join_v1"
    assert contracts.WORKLOAD_FORK_JOIN_SERIAL == "fork_join_serial_v1"


def test_fixtures_tem_5_casos():
    cases_file = PROMPTS_DIR.parent / "fixtures" / "cases.jsonl"
    cases = [json.loads(line) for line in cases_file.read_text().splitlines() if line]
    assert len(cases) == 5
    assert all({"case_id", "entry_text"} <= set(c) for c in cases)


def test_assembly_preserva_ordem_das_mensagens():
    msgs = assembly.build_messages("SYS", "entrada-x", (("ANALISE", "a"), ("REVIEW", "b")))
    assert [m["role"] for m in msgs] == ["system", "user", "user", "user"]
    assert msgs[0]["content"] == "SYS"
    assert msgs[1]["content"] == "ENTRADA: entrada-x"
    assert msgs[2]["content"] == "ANALISE: a"
    assert msgs[3]["content"] == "REVIEW: b"


def test_imports_dos_vendados():
    client = DeterministicClient("http://127.0.0.1:1")
    assert client.base_url == "http://127.0.0.1:1"
    assert callable(control.run_seq_direct)
    assert callable(control.run_forkjoin_direct)
