#!/usr/bin/env bash
# EXP3 (T-880): escalabilidade — c∈{1,2,4,8} workflows concorrentes, série
# principal determinística, single-host (protocolo §3.7.6/Tab.11: 30/nível).
# Uso: run_exp3.sh [runs_por_nivel=30]
set -uo pipefail
RUNS="${1:-30}"
ROOT=/var/mnt/HD1TB/tese
HARN=$ROOT/benchmarks/orchestration
FD=$HOME/.cache/tese-rust-target-fd/release
DOMAIN=188
OUT=$ROOT/src/rust/specs/880-campanha/exp3
mkdir -p "$OUT"

export CYCLONEDDS_URI='file:///var/mnt/HD1TB/tese/config/dds/cyclonedds-test-loopback.xml'
cd $ROOT
echo "== subindo orchestrator + 4 agentes mock (domínio $DOMAIN) =="
pkill -x orchestrator 2>/dev/null; pkill -x agent 2>/dev/null; sleep 0.5
$FD/orchestrator --port 8092 --dds-domain $DOMAIN --qos-manager static >/tmp/e3-orch.log 2>&1 &
for a in 1 2 3 4; do
  $FD/agent --agent-id agent-e3-$a --engine mock --dds-domain $DOMAIN --slots 4 >/tmp/e3-agent$a.log 2>&1 &
done
sleep 3
curl -s http://127.0.0.1:8092/api/v1/agents | python3 -c "import json,sys; print('agentes:', len(json.load(sys.stdin)['agents']))"

for C in 1 2 4 8; do
  echo "== nível c=$C ($RUNS blocos) =="
  : > "$OUT/c$C-latencies.txt"
  START=$(date +%s.%N)
  for blk in $(seq 1 $RUNS); do
    # bloco: C workflows concorrentes (1 wf-run por worker)
    pids=""
    for w in $(seq 1 $C); do
      timeout 60 $FD/wf-run --domain $DOMAIN --workload seq_chain_v1 \
        --entry "exp3 c$C blk$blk w$w" \
        --prompts-dir benchmarks/orchestration/prompts \
        --out "$OUT/.tmp-c$C-b$blk-w$w.json" >/dev/null 2>&1 &
      pids="$pids $!"
    done
    for p in $pids; do wait $p; done
    for w in $(seq 1 $C); do
      f="$OUT/.tmp-c$C-b$blk-w$w.json"
      [[ -f $f ]] && python3 -c "
import json,sys
d=json.load(open('$f'))
if d.get('status')=='completed':
    tot=sum(s['latency_ms'] for s in d['stages'])
    print(tot)
" >> "$OUT/c$C-latencies.txt"
      rm -f "$f"
    done
  done
  END=$(date +%s.%N)
  python3 -c "
import statistics as st
vals=sorted(int(x) for x in open('$OUT/c$C-latencies.txt'))
n=len(vals)
if n:
    print(f'c=$C: n={n} workflows, p50={vals[n//2]}ms p95={vals[int(n*0.95)]}ms media={st.fmean(vals):.0f}ms vazao={n/($END-$START):.1f} wf/s')
"
done
pkill -x orchestrator 2>/dev/null; pkill -x agent 2>/dev/null
echo "fim EXP3"
