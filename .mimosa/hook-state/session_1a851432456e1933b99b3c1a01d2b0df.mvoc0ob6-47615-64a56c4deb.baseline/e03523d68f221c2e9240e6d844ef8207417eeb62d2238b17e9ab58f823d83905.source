#!/usr/bin/env bash
# EXP4 refinado (T-880): injeção log-driven — kill -9 da vítima na 2ª
# "task concluída" (fim de B = início de C, ponto de falha da dissertação).
# Janela do cliente: 60 s (--timeout-ms 60000). Timeout = desfecho de segurança.
# Uso: run_exp4_refinado.sh [runs=30]
set -uo pipefail
RUNS="${1:-30}"
ROOT=/var/mnt/HD1TB/tese
FD=$HOME/.cache/tese-rust-target-fd/release
OUT=$ROOT/src/rust/specs/880-campanha/exp4/refinado
mkdir -p "$OUT"
export CYCLONEDDS_URI='file:///var/mnt/HD1TB/tese/config/dds/cyclonedds-test-loopback.xml'

cd "$ROOT"
pkill -x orchestrator 2>/dev/null; pkill -x agent-vitima 2>/dev/null; pkill -x agent-saudavel 2>/dev/null; pkill -x det-responder 2>/dev/null; sleep 0.5
$FD/orchestrator --port 8093 --dds-domain 189 --qos-manager static >/tmp/e4-orch.log 2>&1 &
$FD/det-responder --domain 189 --delay-ms 50 --capacity 4 --queue 64 --log "$OUT/det.jsonl" >/dev/null 2>&1 &
/tmp/agent-saudavel --agent-id agent-saudavel --engine mock --dds-domain 189 --slots 4 >/tmp/e4-saudavel.log 2>&1 &
sleep 2.5

: > "$OUT/resultados.txt"
for rep in $(seq 1 "$RUNS"); do
  pkill -9 -x agent-vitima 2>/dev/null; sleep 0.4
  : > /tmp/e4-vitima.log
  /tmp/agent-vitima --agent-id agent-vitima --engine mock --dds-domain 189 --slots 4 >/tmp/e4-vitima.log 2>&1 &
  sleep 1.5

  # monitor log-driven: conta "task concluída"; na 2ª (fim de B) mata a vítima
  (
    for _ in $(seq 1 6000); do
      C=$(grep -c 'task concluída' /tmp/e4-vitima.log 2>/dev/null || echo 0)
      if [ "${C:-0}" -ge 2 ]; then
        T_KILL=$(date +%s%N)
        pkill -9 -x agent-vitima 2>/dev/null
        echo "$T_KILL" > /tmp/e4-kill-$rep.txt
        exit 0
      fi
      sleep 0.01
    done
  ) &
  MONPID=$!

  T0=$(date +%s%N)
  timeout 75 $FD/wf-run --domain 189 --workload seq_chain_v1 --entry "exp4 refinado $rep" \
    --timeout-ms 60000 --prompts-dir benchmarks/orchestration/prompts \
    --out "$OUT/wf-$rep.json" >/dev/null 2>&1
  T1=$(date +%s%N)
  wait "$MONPID" 2>/dev/null
  TKILL=$(cat /tmp/e4-kill-$rep.txt 2>/dev/null || echo 0)

  ST=$(python3 -c "import json;print(json.load(open('$OUT/wf-$rep.json'))['status'])" 2>/dev/null || echo timeout-60s)
  if [ "$TKILL" != "0" ] && [ "$T1" != "0" ]; then
    REC=$(( (T1 - TKILL) / 1000000 ))
    echo "rep $rep: status=$ST t_kill_ate_fim=${REC}ms t_total=$(( (T1-T0)/1000000 ))ms" | tee -a "$OUT/resultados.txt"
  else
    echo "rep $rep: status=$ST (falha não injetada ou kill sem timestamp)" | tee -a "$OUT/resultados.txt"
  fi
  rm -f /tmp/e4-kill-$rep.txt
done
pkill -x det-responder 2>/dev/null; pkill -x agent-vitima 2>/dev/null; pkill -x agent-saudavel 2>/dev/null; pkill -x orchestrator 2>/dev/null
echo fim-exp4-refinado
