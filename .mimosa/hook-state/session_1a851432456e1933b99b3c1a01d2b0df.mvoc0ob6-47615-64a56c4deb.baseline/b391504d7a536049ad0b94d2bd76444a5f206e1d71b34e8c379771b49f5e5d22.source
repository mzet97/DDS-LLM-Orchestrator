#!/usr/bin/env python3
"""EXP4 lado MAF (T-880): falha em C + checkpoint/retomada nativa, 30 runs.

Contraparte MAF do EXP4 definitivo DDS (kill da vítima no fim de B):
- mesmo backend determinístico (delay 50 ms, capacity 4 — espelha det-responder);
- mesma cadeia seq_chain_v1 (A→B→C, prompts canônicos);
- falha lógica injetada UMA vez no início de C (antes da inferência — equivale
  ao momento do kill: B concluído, C sem output);
- retomada nativa: checkpoint com saída validada de B → novo workflow resume;
- métrica: t_recovery = t(retomada completa) − t(falha observada).

Executar a partir de benchmarks/orchestration (prompts/ e pacote bench):
  PYTHONPATH=src:.venv/lib/python3.14/site-packages /usr/bin/python3.14 \
    <caminho>/run_exp4_maf.py [runs=30] --out <dir>
"""
from __future__ import annotations

import argparse
import asyncio
import json
import sys
import tempfile
import threading
import time
from pathlib import Path

MAF_MSG_TYPES = ["bench.systems.maf.seq:OutA", "bench.systems.maf.seq:OutB",
                 "bench.systems.maf.seq:OutC"]


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("runs", nargs="?", type=int, default=30)
    ap.add_argument("--out", default=None)
    ap.add_argument("--delay-ms", type=int, default=50)
    ap.add_argument("--capacity", type=int, default=4)
    args = ap.parse_args()
    out_dir = Path(args.out) if args.out else Path(__file__).parent
    out_dir.mkdir(parents=True, exist_ok=True)

    from bench.backend_deterministic.server import serve
    from bench.providers.deterministic import DeterministicClient
    from bench.systems.maf.seq import build_seq_workflow
    from agent_framework import FileCheckpointStorage

    srv, state = serve(port=0, delay_ms=args.delay_ms, capacity=args.capacity)
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    prompts = "prompts"

    async def find_checkpoint_with_b_output(store, workflow_name, b_content):
        cps = await store.list_checkpoints(workflow_name=workflow_name)
        for cp in cps:
            full = await store.load(cp.checkpoint_id)
            for msgs in (full.messages or {}).values():
                for m in msgs or []:
                    data = getattr(m, "data", None)
                    if type(data).__name__ == "OutB" and getattr(data, "content", None) == b_content:
                        return full
        return None

    async def one_rep(rep: int) -> dict:
        rec: dict = {}

        def counts_delta(base: dict) -> dict:
            cur: dict = {}
            for r in state.requests:
                cur[r["stage"]] = cur.get(r["stage"], 0) + 1
            return {k: cur.get(k, 0) - base.get(k, 0) for k in cur}

        base: dict = {}
        for r in state.requests:
            base[r["stage"]] = base.get(r["stage"], 0) + 1
        # 1) nominal (baseline da cadeia íntegra)
        record, shared = [], {}
        wf = build_seq_workflow(DeterministicClient(
            f"http://127.0.0.1:{srv.server_address[1]}"), prompts,
            f"exp4-maf-def {rep}", record, shared, name=f"exp4-maf-n{rep}")
        t0 = time.perf_counter()
        await wf.run("go")
        rec["t_total_nominal_ms"] = round((time.perf_counter() - t0) * 1000)

        # 2) falho: workflow novo (mesmo backend), fault consumida no início de C
        fault = {"consumed": False}
        with tempfile.TemporaryDirectory() as d:
            store = FileCheckpointStorage(d, allowed_checkpoint_types=MAF_MSG_TYPES)
            record, shared = [], {}
            wff = build_seq_workflow(DeterministicClient(
                f"http://127.0.0.1:{srv.server_address[1]}"), prompts,
                f"exp4-maf-def {rep}", record, shared,
                fault_stage="C", fault=fault, name=f"exp4-maf-f{rep}")
            t_fail = time.perf_counter()
            raised = False
            try:
                await wff.run("go", checkpoint_storage=store)
            except RuntimeError as e:
                raised = "falha injetada" in str(e)
            assert raised, f"rep {rep}: falha em C não observada"
            # último B = o da cadeia falhada desta rep (state.requests é cumulativo)
            b_content = next(r["resp"] for r in reversed(state.requests)
                             if r["stage"] == "B")
            chosen = await find_checkpoint_with_b_output(
                store, f"exp4-maf-f{rep}", b_content)
            assert chosen is not None, f"rep {rep}: checkpoint de B não encontrado"
            # 3) retomada nativa a partir do checkpoint com B validado
            record2, shared2 = [], {}
            wf2 = build_seq_workflow(DeterministicClient(
                f"http://127.0.0.1:{srv.server_address[1]}"), prompts,
                f"exp4-maf-def {rep}", record2, shared2,
                fault_stage="C", fault=fault, name=f"exp4-maf-f{rep}")
            r2 = await wf2.run(checkpoint_id=chosen.checkpoint_id,
                               checkpoint_storage=store)
            rec["t_recovery_ms"] = round((time.perf_counter() - t_fail) * 1000)
            rec["resumed_output"] = bool(r2.get_outputs())

        # Delta da rep: A:2/B:2 (nominal + falhada; a RETOMADA não re-executa
        # A/B — propriedade-chave do checkpoint) e C:2 (nominal + retomada;
        # a tentativa falhada aborta ANTES da inferência).
        rec["stage_counts_delta"] = counts_delta(base)
        rec["rep"] = rep
        (out_dir / f"maf-{rep}.json").write_text(json.dumps(rec, indent=1))
        return rec

    lines = []
    for rep in range(1, args.runs + 1):
        rec = asyncio.run(one_rep(rep))
        ok = (rec.get("resumed_output") and
              rec.get("stage_counts_delta") == {"A": 2, "B": 2, "C": 2})
        line = (f"rep {rec['rep']}: {'ok' if ok else 'INVALIDO'} "
                f"t_recovery={rec['t_recovery_ms']}ms "
                f"t_total_nominal={rec['t_total_nominal_ms']}ms")
        print(line, flush=True)
        lines.append(line)

    srv.shutdown()
    recs = [json.loads((out_dir / f"maf-{i}.json").read_text())
            for i in range(1, args.runs + 1)]
    trec = sorted(r["t_recovery_ms"] for r in recs)
    tnom = sorted(r["t_total_nominal_ms"] for r in recs)
    med = lambda v: v[len(v) // 2] if len(v) % 2 else (v[len(v)//2 - 1] + v[len(v)//2]) / 2
    summary = (f"resumo: n={len(recs)} t_recovery_med={med(trec)}ms "
               f"min={trec[0]} max={trec[-1]} | "
               f"t_nominal_med={med(tnom)}ms min={tnom[0]} max={tnom[-1]}")
    print(summary)
    lines.append(summary)
    (out_dir / "resultados.txt").write_text("\n".join(lines) + "\n")
    return 0


if __name__ == "__main__":
    sys.exit(main())
