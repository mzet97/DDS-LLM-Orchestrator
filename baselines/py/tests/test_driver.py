"""Driver ponta-a-ponta (T-907-10 / REQ-907-010): interleave, manifest, JSONL."""

from __future__ import annotations

import json
import threading
from pathlib import Path

import pytest

from baselines.backends.deterministic_server import serve


@pytest.fixture()
def backend():
    srv, state = serve(port=0, delay_ms=0, capacity=8)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    yield srv, state
    srv.shutdown()


def _args(out: Path, url: str, tmp: Path):
    import argparse

    from baselines.driver import PROMPTS_DIR

    return argparse.Namespace(
        systems="langgraph,maf", scenario="seq", runs=2, warmup=1,
        case="caso-04", llama_url=url, model="deterministico", temperature=0.0,
        max_tokens=64, timeout_s=30.0, seed_base=1000, concurrency=2,
        dds_bin=str(tmp / "nao-existe"), dds_domain=79, out=str(out),
        _prompts_dir=PROMPTS_DIR)


def test_driver_interleave_manifest_jsonl(backend, tmp_path):
    from baselines import driver

    srv, _ = backend
    out = tmp_path / "out"
    args = _args(out, f"http://127.0.0.1:{srv.server_address[1]}", tmp_path)
    asyncio_run = __import__("asyncio").run
    asyncio_run(driver.main_async(args))

    lg = out / "langgraph-seq.jsonl"
    maf = out / "maf-seq.jsonl"
    assert lg.exists() and maf.exists()
    recs_lg = [json.loads(l) for l in lg.read_text().splitlines() if l]
    recs_maf = [json.loads(l) for l in maf.read_text().splitlines() if l]
    assert len(recs_lg) == len(recs_maf) == 3  # 1 warmup + 2 runs
    assert [r["warmup"] for r in recs_lg] == [True, False, False]
    assert all(r["status"] == "completed" for r in recs_lg + recs_maf)
    assert all(len(r["stages"]) == 3 for r in recs_lg + recs_maf)
    # interleave round-robin: run_index e seed idênticos entre sistemas por rodada
    assert [r["run_index"] for r in recs_lg] == [r["run_index"] for r in recs_maf]
    assert [r["seed"] for r in recs_lg] == [r["seed"] for r in recs_maf]
    # métricas por estágio via TimingPort
    assert len(recs_lg[1]["stage_timings"]) == 3

    manifest = json.loads((out / "manifest.json").read_text())
    assert manifest["systems"] == ["langgraph", "maf"]
    assert manifest["runs"] == 2 and manifest["warmup"] == 1

    # análise ponta-a-ponta sobre a saída do driver
    from baselines.analysis import summarize, tost_paired
    s = summarize(lg)
    assert s["n"] == 2 and s["p50_s"] > 0
    r = tost_paired([r["t_total_s"] for r in recs_lg if not r["warmup"]],
                    [r["t_total_s"] for r in recs_maf if not r["warmup"]],
                    mei=0.020)
    assert r["verdict"] in ("equivalente", "NAO-equivalente", "inconclusivo")
