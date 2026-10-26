"""Driver da campanha (T-907-10 / REQ-907-010): 1+ sistemas × cenário × n runs.

Ordem intercalada round-robin entre sistemas (drift térmico igual); warmup
descartado na análise mas gravado; seed por run (reprodutibilidade); JSONL por
sistema + manifest.json. Uso:

  python -m baselines.driver --systems langgraph,maf,dds --scenario seq \
      --runs 30 --warmup 10 --llama-url http://127.0.0.1:8083 --model gemma-4-12b \
      --out ~/.cache/tese-results-907/exp1-seq/ \
      [--dds-bin PATH --dds-domain 79] [--concurrency 4] [--case caso-04]
"""

from __future__ import annotations

import argparse
import asyncio
import json
import platform
import time
from datetime import datetime, timezone
from pathlib import Path

from baselines.contracts import WorkflowInput
from baselines.ports import CallCounterPort, FaultyPort, TimingPort
from baselines.providers.llama import LlamaClient
from baselines.scenarios.concurrency import (run_concurrency_langgraph,
                                             run_concurrency_maf)
from baselines.scenarios.recovery import run_recovery_langgraph, run_recovery_maf
from baselines.scenarios.stream import stream_langgraph, stream_maf
from baselines.systems.dds_rust import (DdsSubmitOne, run_concurrency_dds,
                                        run_forkjoin_dds, run_seq_dds)
from baselines.systems.langgraph.forkjoin import run_forkjoin_langgraph
from baselines.systems.langgraph.seq import run_seq_langgraph
from baselines.systems.maf.forkjoin import run_forkjoin_maf
from baselines.systems.maf.seq import run_seq_maf

PROMPTS_DIR = str(Path(__file__).resolve().parents[1] / "baselines" / "prompts")
FIXTURES = Path(__file__).resolve().parents[1] / "baselines" / "fixtures" / "cases.jsonl"

SCENARIOS = ("seq", "forkjoin", "serial", "concurrency", "stream", "recovery")


def load_entry(case_id: str) -> str:
    for line in FIXTURES.read_text().splitlines():
        if line.strip():
            c = json.loads(line)
            if c["case_id"] == case_id:
                return c["entry_text"]
    raise SystemExit(f"case {case_id} não está em fixtures/cases.jsonl")


def make_port(system: str, args, seed: int):
    """Porta por sistema; seed por run (Llama) / parâmetros (DDS)."""
    if system in ("langgraph", "maf"):
        client = LlamaClient(args.llama_url, args.model, temperature=args.temperature,
                             seed=seed, max_tokens=args.max_tokens,
                             timeout_s=args.timeout_s)
        return TimingPort(_Llama(client))
    if system == "dds":
        return TimingPort(DdsSubmitOne(args.dds_bin, args.dds_domain, args.model,
                                       max_tokens=args.max_tokens,
                                       temperature=args.temperature))
    raise SystemExit(f"sistema desconhecido: {system}")


class _Llama:
    """Adapter mínimo LlamaClient → porta (stage/expects ignorados)."""

    def __init__(self, client: LlamaClient):
        self._c = client

    async def acomplete(self, messages, stage, expects=()):
        return await self._c.acomplete(messages)


async def run_one(system: str, scenario: str, args, port, entry: str,
                  run_idx: int) -> dict:
    wf = WorkflowInput(case_id=args.case,
                       workflow_id=f"{system}-{scenario}-{args.case}-r{run_idx:03d}",
                       workload_id={"serial": "fork_join_serial_v1",
                                    "forkjoin": "fork_join_v1"}.get(scenario,
                                                                    "seq_chain_v1"))
    t0 = time.monotonic()
    rec: dict = {"run_index": run_idx, "system": system, "scenario": scenario,
                 "case_id": args.case, "workflow_id": wf.workflow_id,
                 "seed": args.seed_base + run_idx, "warmup": run_idx < args.warmup,
                 "t_start_monotonic": t0}
    try:
        if scenario == "seq":
            fn = {"langgraph": run_seq_langgraph, "maf": run_seq_maf,
                  "dds": run_seq_dds}[system]
            r = await fn(wf, port, PROMPTS_DIR, entry)
        elif scenario == "forkjoin":
            fn = {"langgraph": run_forkjoin_langgraph, "maf": run_forkjoin_maf,
                  "dds": run_forkjoin_dds}[system]
            if system == "langgraph":
                r = await fn(wf, port, PROMPTS_DIR, entry, limit_p=2)
            elif system == "maf":
                r = await fn(wf, port, PROMPTS_DIR, entry, serial=False)
            else:
                r = await fn(wf, port, PROMPTS_DIR, entry, serial=False)
        elif scenario == "serial":
            fn = {"langgraph": run_forkjoin_langgraph, "maf": run_forkjoin_maf,
                  "dds": run_forkjoin_dds}[system]
            if system == "langgraph":
                r = await fn(wf, port, PROMPTS_DIR, entry, limit_p=1)
            else:
                r = await fn(wf, port, PROMPTS_DIR, entry, serial=True)
        elif scenario == "stream":
            if system == "dds":
                raise SystemExit("stream DDS: usar chunks de TaskOutput via agent (T-907-09 not applicable aqui); cenário 907 cobre direct/LG/MAF")
            base = {"langgraph": stream_langgraph, "maf": stream_maf}[system]
            rep = await base(wf, port, PROMPTS_DIR, entry)
            rec.update(ttfc_s=rep.ttfc_s, granularity=rep.granularity,
                       text_head=rep.text[:80])
            r = None
        elif scenario == "concurrency":
            if system == "dds":
                rep = await run_concurrency_dds(wf, port, PROMPTS_DIR, entry,
                                                c=args.concurrency)
            else:
                base = {"langgraph": run_concurrency_langgraph,
                        "maf": run_concurrency_maf}[system]
                rep = await base(wf, port, PROMPTS_DIR, entry, c=args.concurrency)
            rec.update(completed=rep["completed"], failed=rep["failed"],
                       makespan_s=rep["makespan_s"],
                       per_workflow_s=rep["per_workflow_s"], errors=rep["errors"])
            r = None
        elif scenario == "recovery":
            if system == "dds":
                raise SystemExit("recovery DDS = protocolo EXP4/880 congelado "
                                 "(reaper/orchestrator); ver REPORT")
            client = _Llama(LlamaClient(args.llama_url, args.model,
                                        temperature=args.temperature,
                                        seed=args.seed_base + run_idx,
                                        max_tokens=args.max_tokens,
                                        timeout_s=args.timeout_s))
            counter = CallCounterPort(client)
            port = FaultyPort(counter, fail_on_stage="C", times=1)
            if system == "langgraph":
                rep = await run_recovery_langgraph(wf, port, PROMPTS_DIR, entry,
                                                   counter)
            else:
                maf_dir = Path(args.out) / "maf-ckpt"
                maf_dir.mkdir(parents=True, exist_ok=True)
                rep = await run_recovery_maf(wf, port, PROMPTS_DIR, entry,
                                             str(maf_dir), counter)
            rec["status"] = ("completed" if (rep.failed_first_run
                                             and rep.completed_on_resume) else "failed")
            rec.update(failed_first_run=rep.failed_first_run,
                       completed_on_resume=rep.completed_on_resume,
                       stage_counts=rep.stage_counts, t_fail_s=rep.t_fail_s,
                       t_recover_s=rep.t_recover_s, detail=rep.detail)
            r = None
        else:
            raise SystemExit(f"cenário desconhecido: {scenario}")
        rec["t_total_s"] = time.monotonic() - t0
        if "status" not in rec:
            rec["status"] = "completed" if r is None else r.status
        if r is not None:
            rec["stages"] = [{"step_id": s.step_id,
                              "tokens_prompt": s.tokens_prompt,
                              "tokens_completion": s.tokens_completion}
                             for s in r.stages]
            rec["content_head"] = {s.step_id: s.content[:64] for s in r.stages}
    except Exception as e:  # falha é DADO, não crash do driver
        rec["t_total_s"] = time.monotonic() - t0
        rec["status"] = "failed"
        rec["error"] = str(e)[:300]
    port_records = getattr(port, "records", None)
    if port_records is not None:
        rec["stage_timings"] = [dict(r) for r in port_records]
        port.records = []
    return rec


async def main_async(args):
    entry = load_entry(args.case)
    systems = [s.strip() for s in args.systems.split(",") if s.strip()]
    out = Path(args.out)
    out.mkdir(parents=True, exist_ok=True)
    files = {s: open(out / f"{s}-{args.scenario}.jsonl", "a") for s in systems}
    manifest = {
        "created_utc": datetime.now(timezone.utc).isoformat(),
        "scenario": args.scenario, "systems": systems, "runs": args.runs,
        "warmup": args.warmup, "case": args.case, "model": args.model,
        "llama_url": args.llama_url, "temperature": args.temperature,
        "max_tokens": args.max_tokens, "seed_base": args.seed_base,
        "concurrency": args.concurrency, "dds_domain": args.dds_domain,
        "python": platform.python_version(), "prompts_dir": PROMPTS_DIR,
    }
    for i in range(args.runs + args.warmup):
        for s in systems:  # round-robin entre sistemas (drift térmico igual)
            port = make_port(s, args, seed=args.seed_base + i)
            rec = await run_one(s, args.scenario, args, port, entry, i)
            files[s].write(json.dumps(rec, ensure_ascii=False) + "\n")
            files[s].flush()
            print(f"[{i + 1}/{args.runs + args.warmup}] {s} {args.scenario}: "
                  f"{rec['status']} {rec['t_total_s']:.2f}s", flush=True)
    for f in files.values():
        f.close()
    (out / "manifest.json").write_text(json.dumps(manifest, ensure_ascii=False,
                                                  indent=2))
    print(f"ok: {out}/")


def main():
    p = argparse.ArgumentParser(prog="baselines.driver")
    p.add_argument("--systems", required=True,
                   help="lista separada por vírgulas: langgraph,maf,dds")
    p.add_argument("--scenario", required=True, choices=SCENARIOS)
    p.add_argument("--runs", type=int, default=30)
    p.add_argument("--warmup", type=int, default=10)
    p.add_argument("--case", default="caso-04")
    p.add_argument("--llama-url", default="http://127.0.0.1:8083")
    p.add_argument("--model", default="gemma-4-12b")
    p.add_argument("--temperature", type=float, default=0.0)
    p.add_argument("--max-tokens", type=int, default=512)
    p.add_argument("--timeout-s", type=float, default=180.0)
    p.add_argument("--seed-base", type=int, default=1000)
    p.add_argument("--concurrency", type=int, default=4)
    p.add_argument("--dds-bin", default="submit-one")
    p.add_argument("--dds-domain", type=int, default=79)
    p.add_argument("--out", required=True)
    asyncio.run(main_async(p.parse_args()))


if __name__ == "__main__":
    main()
