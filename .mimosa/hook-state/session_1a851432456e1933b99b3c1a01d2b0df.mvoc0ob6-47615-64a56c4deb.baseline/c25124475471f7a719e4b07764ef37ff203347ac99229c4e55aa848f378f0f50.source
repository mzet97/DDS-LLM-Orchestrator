#!/usr/bin/env bash
# EXP1 (T-880): overhead de orquestração — 30 runs/condição/sistema,
# condições de atraso {0,50} ms, + controle sem orquestrador.
# Uso: run_exp1.sh <delay_ms> <runs> <outdir>
set -uo pipefail
DELAY="${1:?delay_ms}"; RUNS="${2:-30}"; OUT="${3:?outdir}"
ROOT=/var/mnt/HD1TB/tese
HARN=$ROOT/benchmarks/orchestration
FD=$HOME/.cache/tese-rust-target-fd/release
PY=$HOME/.cache/tese-venv-exp0/bin/python
DOMAIN=187
mkdir -p "$OUT"/{maf,langgraph,dds,controle}

cd "$HARN"
echo "== MAF/LangGraph (harness, backend in-process delay=$DELAY) =="
for i in $(seq 1 $RUNS); do
  PYTHONPATH=$HARN/src $PY -m bench.cli run --system maf      --workload seq_chain_v1 --n 1 --delay-ms $DELAY --capacity 4 --out "$OUT/.tmp-maf" >/dev/null 2>&1 \
    && cp "$OUT"/.tmp-maf/maf-seq_chain_v1-n1-*/metrics.json "$OUT/maf/run$i.json" 2>/dev/null
  PYTHONPATH=$HARN/src $PY -m bench.cli run --system langgraph --workload seq_chain_v1 --n 1 --delay-ms $DELAY --capacity 4 --out "$OUT/.tmp-lg" >/dev/null 2>&1 \
    && cp "$OUT"/.tmp-lg/langgraph-seq_chain_v1-n1-*/metrics.json "$OUT/langgraph/run$i.json" 2>/dev/null
  rm -rf "$OUT"/.tmp-maf "$OUT"/.tmp-lg
done
echo "maf: $(ls $OUT/maf | wc -l) lg: $(ls $OUT/langgraph | wc -l)"

echo "== DDS (wf-run + agent engine=dds + det-responder delay=$DELAY) =="
export CYCLONEDDS_URI='file:///var/mnt/HD1TB/tese/config/dds/cyclonedds-test-loopback.xml'
pkill -x det-responder 2>/dev/null; pkill -x agent 2>/dev/null; sleep 0.5
$FD/det-responder --domain $DOMAIN --delay-ms $DELAY --capacity 4 --queue 64 --log "$OUT/det-delay$DELAY.jsonl" >/dev/null 2>&1 &
$FD/agent --agent-id agent-exp1 --engine dds --dds-domain $DOMAIN --slots 4 >/dev/null 2>&1 &
sleep 2
cd $ROOT
ENTRY=$(python3 -c "import json;print(json.loads(open('$HARN/fixtures/cases.jsonl').readline())['entry_text'])")
for i in $(seq 1 $RUNS); do
  timeout 60 $FD/wf-run --domain $DOMAIN --workload seq_chain_v1 --entry "$ENTRY" \
    --prompts-dir benchmarks/orchestration/prompts --out "$OUT/dds/run$i.json" >/dev/null 2>&1
done
pkill -x det-responder 2>/dev/null; pkill -x agent 2>/dev/null
echo "dds: $(ls $OUT/dds 2>/dev/null | wc -l)"

echo "== Controle (sem orquestrador: 3 chamadas sequenciais no mesmo processo) =="
DELAY=$DELAY RUNS=$RUNS OUT="$OUT/controle" PYTHONPATH=$HARN/src python3 - <<'EOF'
import json, os, sys, time
sys.path.insert(0, os.environ["PYTHONPATH"])
from bench.backend_deterministic.server import serve
delay = int(os.environ["DELAY"]); runs = int(os.environ["RUNS"])
for i in range(1, runs + 1):
    srv, state = serve(port=0, delay_ms=delay, capacity=4)
    import threading
    threading.Thread(target=srv.serve_forever, daemon=True).start()
    import urllib.request
    port = srv.server_address[1]
    t0 = time.monotonic()
    for j in range(3):
        body = json.dumps({"model":"qwen3.5-0.8b","messages":[{"role":"user","content":f"ctrl {j}"}],"max_tokens":8}).encode()
        req = urllib.request.Request(f"http://127.0.0.1:{port}/v1/chat/completions", data=body, headers={"content-type":"application/json"})
        urllib.request.urlopen(req, timeout=30).read()
    total = time.monotonic() - t0
    srv.shutdown()
    backend = sum(r["finished_ns"] - r["started_ns"] for r in state.requests) / 1e9
    json.dump({"run": i, "t_total_s": round(total, 4), "t_backend_sum_s": round(backend, 4)},
              open(f"{os.environ['OUT']}/run{i}.json", "w"))
print("controle ok")
EOF
echo "== resumo $OUT =="
$PY - "$OUT" <<'EOF'
import json, glob, statistics as st, sys, os
out = sys.argv[1]
for sysname in ("maf", "langgraph", "dds", "controle"):
    totals, backs = [], []
    for f in sorted(glob.glob(f"{out}/{sysname}/run*.json")):
        d = json.load(open(f))
        if sysname == "dds":
            stages = json.load(open(f))
            total = sum(s["latency_ms"] for s in stages.get("stages", [])) / 1000
            totals.append(total); backs.append(3 * 0.050)
        else:
            totals.append(d["t_total_s"]); backs.append(d["t_backend_sum_s"])
    if totals:
        extras = [t - b for t, b in zip(totals, backs)]
        print(f"{sysname:10s} n={len(totals):2d} T_total_medio={st.fmean(totals):.3f}s "
              f"T_backend_medio={st.fmean(backs):.3f}s T_extra_medio={st.fmean(extras):.3f}s "
              f"T_extra_p50={sorted(extras)[len(extras)//2]:.3f}s")
EOF
