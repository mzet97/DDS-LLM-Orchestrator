#!/bin/bash
# Canário descartável do ciclo deploy→reinício→validação→falha→rollback.
#
# Demonstra com o BINÁRIO REAL em diretório próprio, porta de teste e DB
# temporário: estado não vazio, instalação v2, reinício, validação pelo
# esquema real, falha controlada (binário corrompido) e rollback para v1.
# Processos criados têm PID registrado e limpeza garantida (trap).
# v1 = binário release atual; v2 = build debug (artefato DISTINTO).
set -euo pipefail

ROOT="$(cd "$(dirname "$0")/../.." && pwd)"
WORK="$(mktemp -d /tmp/canario-XXXXXX)"
PORT="$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')"
BIN_V1="$WORK/v1/studio-noded"
BIN_V2="$WORK/v2/studio-noded"
DEPLOY="$WORK/deploy"
DB="$DEPLOY/studio-node-log.json"
PIDS=()

cleanup() {
  for pid in "${PIDS[@]:-}"; do kill "$pid" 2>/dev/null || true; done
  cp "$WORK"/node-*.log /tmp/ 2>/dev/null || true
  rm -rf "$WORK"
}
trap cleanup EXIT

echo "== canário descartável em $WORK"
mkdir -p "$WORK/v1" "$WORK/v2" "$DEPLOY"
cp "$ROOT/target/release/studio-noded" "$BIN_V1"
cp "$ROOT/target/debug/studio-noded" "$BIN_V2" 2>/dev/null || \
  (cd "$ROOT" && cargo build --locked -p studio-node 2>/dev/null && cp target/debug/studio-noded "$BIN_V2")
test "$(sha256sum < "$BIN_V1" | cut -d' ' -f1)" != "$(sha256sum < "$BIN_V2" | cut -d' ' -f1)" \
  || { echo "v1 e v2 idênticos: rollback sem contraste"; exit 1; }
H1="$(sha256sum < "$BIN_V1" | cut -d' ' -f1)"

start_node() {
  STUDIO_NODE_PORT=$PORT STUDIO_NODE_DB="$DB" "$1" >"$WORK/node-$PORT.log" 2>&1 & echo $!
}

stop_node() { # $1=pid: SIGTERM + espera ativa (job nasceu em subshell; wait não o enxerga)
  kill "$1" 2>/dev/null || return 0
  for _ in $(seq 1 30); do kill -0 "$1" 2>/dev/null || return 0; sleep 1; done
  echo "node $1 não terminou após SIGTERM"; return 1
}

echo "-- estado não vazio em v1"
cp "$BIN_V1" "$DEPLOY/studio-noded"
PIDS+=("$(start_node "$DEPLOY/studio-noded")")
for _ in $(seq 1 30); do curl -sf "http://127.0.0.1:$PORT/version" >/dev/null && break; sleep 1; done
curl -sf -X POST "http://127.0.0.1:$PORT/apply" -H 'Content-Type: application/json' \
  -d '{"protocol":{"major":1,"minor":0},"operation_id":"canario-1","op":{"kind":"set_service","service":"dds-agent","running":true}}' \
  | grep -q '"outcome":"applied"'
test "$(python3 -c "import json;print(len(json.load(open('$DB'))['records']))")" = "1"

echo "-- backup explícito + instalação v2 + reinício"
cp "$DB" "$WORK/backup-v1.json"
stop_node "${PIDS[0]}"
cp "$BIN_V2" "$DEPLOY/studio-noded"
PIDS=("$(start_node "$DEPLOY/studio-noded")")
for _ in $(seq 1 30); do curl -sf "http://127.0.0.1:$PORT/version" >/dev/null && break; sleep 1; done
test "$(sha256sum < "$DEPLOY/studio-noded" | cut -d' ' -f1)" != "$H1"
python3 "$ROOT/ansible/validate_db.py" "$DB" "$WORK/backup-v1.json"

echo "-- falha controlada: binário corrompido NÃO passa"
stop_node "${PIDS[0]}"
echo corrompido >> "$DEPLOY/studio-noded"
if "$ROOT/ansible/validate_db.py" "$DB" "$WORK/backup-v1.json" >/dev/null 2>&1 \
   && test "$(sha256sum < "$DEPLOY/studio-noded" | cut -d' ' -f1)" = "$H1"; then
  echo "FALHA CONTROLADA NÃO DETECTADA"; exit 1
fi
echo "hash diverge do aprovado: falha detectada como esperado"

echo "-- falha de boot de versao instalada + rollback com estado"
cp /bin/false "$DEPLOY/studio-noded"
PIDS=("$(start_node "$DEPLOY/studio-noded")")
booted=0
for _ in $(seq 1 5); do curl -sf "http://127.0.0.1:$PORT/version" >/dev/null && booted=1 && break; sleep 1; done
stop_node "${PIDS[0]}"
test "$booted" = "0" || { echo "VERSAO QUEBRADA SUBIU (ruim)"; exit 1; }
echo "versao instalada nao inicializa: rollback com estado preservado"

echo "-- rollback para v1 + validação"
cp "$BIN_V1" "$DEPLOY/studio-noded"
test "$(sha256sum < "$DEPLOY/studio-noded" | cut -d' ' -f1)" = "$H1"
PIDS=("$(start_node "$DEPLOY/studio-noded")")
for _ in $(seq 1 30); do curl -sf "http://127.0.0.1:$PORT/version" >/dev/null && break; sleep 1; done
python3 "$ROOT/ansible/validate_db.py" "$DB" "$WORK/backup-v1.json"
test "$(curl -s "http://127.0.0.1:$PORT/version")" = '{"major":1,"minor":0}'
echo "CANÁRIO OK: deploy, reinício, validação, falha e rollback demonstrados"
