#!/usr/bin/env bash
# Harness de carga multi-processo (Fase R1 do OPTIMIZATION_PLAN.md — "Fases pendentes,
# Rodada 2"). Sobe, no MESMO domínio DDS: policy-engine, context-store, mcp-gateway,
# observability, orchestrator e N agentes reais (--engine mock, para não depender de
# llama-server/modelo — o alvo aqui é a camada de coordenação DDS, não a inferência),
# depois roda o gerador de carga real (`dds-bench`, cenário OP1 closed-loop) com M
# clientes concorrentes.
#
# Usado por:
#   - R1: prova que o cenário sobe e troca dados de ponta a ponta.
#   - R2: como bônus, já imprime a contagem de threads de cada processo em pico de carga
#     (via /proc/<pid>/status) — a métrica que a Fase 5 (WaitSet compartilhado) precisava
#     medir sob carga real, e que não dava para obter sem esta infraestrutura.
#
# Uso:
#   CYCLONEDDS_STATIC=1 ./multiprocess_load_harness.sh [domain] [duration_s] [n_agents] [n_clients]
#
# Requer CARGO_TARGET_DIR fora do mount SMB/CIFS (setar antes de chamar, ou deixa o
# default abaixo).

set -uo pipefail

DOMAIN="${1:-90}"
DURATION_S="${2:-30}"
N_AGENTS="${3:-3}"
N_CLIENTS="${4:-20}"

export CARGO_TARGET_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/tese-rust-target}"
export CYCLONEDDS_STATIC=1

cd "$(dirname "${BASH_SOURCE[0]}")/.."  # tese/src/rust

# Os binários lincam dinamicamente contra libddsc.so.11 (CYCLONEDDS_STATIC afeta o
# build, não elimina essa dependência em runtime) — sem isto no LD_LIBRARY_PATH,
# todo binário falha com "error while loading shared libraries".
DDSC_LIB_DIR=$(find "$CARGO_TARGET_DIR/debug/build" -maxdepth 6 -iname "libddsc.so*" 2>/dev/null \
    | head -1 | xargs -r dirname)
if [ -z "$DDSC_LIB_DIR" ]; then
    echo "[harness] AVISO: libddsc.so não encontrado em $CARGO_TARGET_DIR/debug/build — build ainda vai rodar, mas os binários provavelmente vão falhar em runtime"
else
    export LD_LIBRARY_PATH="$DDSC_LIB_DIR:${LD_LIBRARY_PATH:-}"
    echo "[harness] LD_LIBRARY_PATH inclui $DDSC_LIB_DIR"
fi

RUN_DIR="$(mktemp -d /tmp/dds-load-harness-XXXXXX)"
echo "[harness] domain=$DOMAIN duration=${DURATION_S}s n_agents=$N_AGENTS n_clients=$N_CLIENTS"
echo "[harness] logs em $RUN_DIR"

declare -a PIDS=()
declare -A PID_NAME=()

cleanup() {
    echo "[harness] encerrando processos..."
    for pid in "${PIDS[@]}"; do
        kill -TERM "$pid" 2>/dev/null
    done
    sleep 1
    for pid in "${PIDS[@]}"; do
        kill -KILL "$pid" 2>/dev/null
    done
    wait 2>/dev/null
    echo "[harness] encerrado."
}
trap cleanup EXIT INT TERM

spawn() {
    local name="$1"; shift
    echo "[harness] subindo $name: $*"
    "$@" > "$RUN_DIR/$name.log" 2>&1 &
    local pid=$!
    PIDS+=("$pid")
    PID_NAME["$pid"]="$name"
}

echo "[harness] build (pode demorar na 1ª vez)..."
cargo build -p policy-engine -p context-store -p mcp-gateway -p observability \
    -p orchestrator -p agent -p benchmarks --features dds \
    > "$RUN_DIR/build.log" 2>&1
if [ $? -ne 0 ]; then
    echo "[harness] build falhou — ver $RUN_DIR/build.log"
    tail -40 "$RUN_DIR/build.log"
    exit 1
fi
BIN="$CARGO_TARGET_DIR/debug"

spawn policy-engine "$BIN/policy-engine" --dds-domain "$DOMAIN" \
    --policy-file "$(pwd)/crates/policy-engine/policies.json" --log-level warn
spawn context-store "$BIN/context-store" --dds-domain "$DOMAIN" \
    --data-file "$RUN_DIR/context_store.jsonl" --log-level warn
spawn mcp-gateway "$BIN/mcp-gateway" --dds-domain "$DOMAIN" \
    --filesystem-root "$RUN_DIR"
spawn observability "$BIN/observability-collector" --dds-domain "$DOMAIN" \
    --output-dir "$RUN_DIR"
spawn orchestrator "$BIN/orchestrator" --dds-domain "$DOMAIN" --qos-manager nfcm

for i in $(seq 1 "$N_AGENTS"); do
    spawn "agent-$i" "$BIN/agent" --agent-id "agent-mock-$i" --dds-domain "$DOMAIN" \
        --slots 8 --engine mock
done

echo "[harness] settle (3s) para os processos subirem e se descobrirem via SEDP..."
sleep 3

echo "[harness] snapshot de threads ANTES da carga:"
for pid in "${PIDS[@]}"; do
    name="${PID_NAME[$pid]}"
    threads=$(awk '/^Threads:/{print $2}' "/proc/$pid/status" 2>/dev/null || echo "?")
    echo "  $name (pid $pid): $threads threads"
done

echo "[harness] gerando carga real: dds-bench OP1, $N_CLIENTS clientes concorrentes, ${DURATION_S}s"
"$BIN/dds-bench" --scenario OP1 --domain "$DOMAIN" --duration "$DURATION_S" \
    --workers "$N_CLIENTS" --arm nfcm --out "$RUN_DIR/bench_out" \
    > "$RUN_DIR/dds-bench.log" 2>&1 &
BENCH_PID=$!

echo "[harness] snapshot de threads DURANTE a carga (após 3s de carga):"
sleep 3
for pid in "${PIDS[@]}"; do
    name="${PID_NAME[$pid]}"
    threads=$(awk '/^Threads:/{print $2}' "/proc/$pid/status" 2>/dev/null || echo "?")
    echo "  $name (pid $pid): $threads threads"
done

wait "$BENCH_PID"
BENCH_STATUS=$?
echo "[harness] dds-bench saiu com status $BENCH_STATUS"
tail -5 "$RUN_DIR/dds-bench.log"

echo "[harness] snapshot de threads APÓS a carga:"
for pid in "${PIDS[@]}"; do
    name="${PID_NAME[$pid]}"
    threads=$(awk '/^Threads:/{print $2}' "/proc/$pid/status" 2>/dev/null || echo "?")
    echo "  $name (pid $pid): $threads threads"
done

echo "[harness] logs preservados em: $RUN_DIR (não apagado automaticamente)"
trap - EXIT
cleanup
