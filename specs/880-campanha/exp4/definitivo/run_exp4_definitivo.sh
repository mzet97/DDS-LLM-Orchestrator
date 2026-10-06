#!/usr/bin/env bash
# EXP4 definitivo (T-880): injeção log-driven COM vítima forçada a claimar.
#
# Diferenças para o refinado:
#   - wf-run --target-agent agent-vitima  → toda task nasce direcionada;
#   - agent-saudavel --target-agent-prefix agent-saudavel → NÃO compete pelos
#     tasks direcionados; só herda APÓS o reaper reatribuir (que limpa
#     target_agent — commit 25998a1). Injeção deixa de depender da distribuição
#     racy de tasks (6/30 → esperado ~30/30).
# Protocolo: kill -9 da vítima na 2ª "task concluída" (fim de B = início de C,
# ponto de falha da dissertação §3.7.7); janela do cliente 60 s; timeout =
# desfecho de segurança. Domínio 189, loopback, engine mock + det-responder.
# Uso: run_exp4_definitivo.sh [runs=30] [smoke=N]
set -uo pipefail
RUNS="${1:-30}"
SMOKE="${2:-0}"
ROOT=/var/mnt/HD1TB/tese
FD=$HOME/.cache/tese-rust-target-fd/release
OUT=$ROOT/src/rust/specs/880-campanha/exp4/definitivo
mkdir -p "$OUT"
export CYCLONEDDS_URI='file:///var/mnt/HD1TB/tese/config/dds/cyclonedds-test-loopback.xml'

cd "$ROOT"
cp "$FD/agent" /tmp/agent-saudavel
cp "$FD/agent" /tmp/agent-vitima
pkill -x orchestrator 2>/dev/null; pkill -x agent-vitima 2>/dev/null
pkill -x agent-saudavel 2>/dev/null; pkill -x det-responder 2>/dev/null; sleep 0.5

$FD/orchestrator --port 8093 --dds-domain 189 --qos-manager static >/tmp/e4d-orch.log 2>&1 &
$FD/det-responder --domain 189 --delay-ms 50 --capacity 4 --queue 64 --log "$OUT/det.jsonl" >/dev/null 2>&1 &
/tmp/agent-saudavel --agent-id agent-saudavel --engine mock --dds-domain 189 --slots 4 \
  --target-agent-prefix agent-saudavel >/tmp/e4d-saudavel.log 2>&1 &
sleep 2.5

: > "$OUT/resultados.txt"
END_REP=$(( SMOKE > 0 ? SMOKE : RUNS ))
for rep in $(seq 1 "$END_REP"); do
  pkill -9 -x agent-vitima 2>/dev/null; sleep 0.4
  : > /tmp/e4d-vitima.log
  /tmp/agent-vitima --agent-id agent-vitima --engine mock --dds-domain 189 --slots 4 >/tmp/e4d-vitima.log 2>&1 &
  sleep 1.5

  # monitor log-driven: na 2ª task concluída da vítima (fim de B), mata o processo
  (
    for _ in $(seq 1 9000); do
      C=$(grep -c 'task concluída' /tmp/e4d-vitima.log 2>/dev/null || true)
      if [ "${C:-0}" -ge 2 ]; then
        date +%s%N > /tmp/e4d-kill-$rep.txt
        pkill -9 -x agent-vitima 2>/dev/null
        exit 0
      fi
      sleep 0.01
    done
  ) &
  MONPID=$!

  T0=$(date +%s%N)
  timeout 75 $FD/wf-run --domain 189 --workload seq_chain_v1 --entry "exp4 definitivo $rep" \
    --timeout-ms 60000 --prompts-dir benchmarks/orchestration/prompts \
    --target-agent agent-vitima \
    --out "$OUT/wf-$rep.json" >/dev/null 2>&1
  T1=$(date +%s%N)
  wait "$MONPID" 2>/dev/null
  TKILL=$(cat /tmp/e4d-kill-$rep.txt 2>/dev/null || echo 0)

  ST=$(python3 -c "import json;print(json.load(open('$OUT/wf-$rep.json'))['status'])" 2>/dev/null || echo timeout-60s)
  if [ "$TKILL" != "0" ]; then
    if [ "$T1" -gt "$TKILL" ]; then
      REC=$(( (T1 - TKILL) / 1000000 ))
      echo "rep $rep: status=$ST injecao=midflight t_kill_ate_fim=${REC}ms t_total=$(( (T1-T0)/1000000 ))ms" | tee -a "$OUT/resultados.txt"
    else
      echo "rep $rep: status=$ST injecao=pos-conclusao t_total=$(( (T1-T0)/1000000 ))ms" | tee -a "$OUT/resultados.txt"
    fi
  else
    echo "rep $rep: status=$ST injecao=nao-disparada t_total=$(( (T1-T0)/1000000 ))ms" | tee -a "$OUT/resultados.txt"
  fi
  rm -f /tmp/e4d-kill-$rep.txt
done
pkill -x det-responder 2>/dev/null; pkill -x agent-vitima 2>/dev/null
pkill -x agent-saudavel 2>/dev/null; pkill -x orchestrator 2>/dev/null
echo fim-exp4-definitivo
