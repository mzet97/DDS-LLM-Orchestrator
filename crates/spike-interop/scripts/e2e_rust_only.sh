#!/usr/bin/env bash
# T-430: E2E Rust-only — HTTP → orchestrator → mesh → agente (DdsEngine) →
# llama-server C++ → resultado de volta na malha. Valida o caminho de produção.
#
# Uso: ./e2e_rust_only.sh [dominio] [porta_http]
set -u

DOMAIN="${1:-105}"
PORT="${2:-8095}"
D=/home/mzet/.cache/tese-rust-target/debug
MODEL=/run/host/var/mnt/HD1TB/tese/models/Phi-4-mini-instruct-Q4_K_M.gguf

unset CYCLONEDDS_URI
export RUST_LOG=info

echo "=== T-430: E2E Rust-only (domínio $DOMAIN, http :$PORT) ==="

# 0) limpa processos anteriores
pkill -f "llama-server.*dds-domain $DOMAIN" 2>/dev/null; sleep 1

# 1) llama-server C++ com DDS
/home/mzet/.cache/llama-build/bin/llama-server -m "$MODEL" \
    --enable-dds --dds-domain "$DOMAIN" -c 2048 --port $((PORT+100)) --host 127.0.0.1 \
    > /tmp/e2e_llama.log 2>&1 &
LLAMA_PID=$!

# 2) orchestrator Rust (API + registry monitor + control loop NFCM)
"$D/orchestrator" --port "$PORT" --dds-domain "$DOMAIN" > /tmp/e2e_orq.log 2>&1 &
ORQ_PID=$!

# 3) agente Rust com DdsEngine (llama-server real)
"$D/agent" --agent-id agent-rust-e2e --dds-domain "$DOMAIN" --engine dds --slots 4 \
    > /tmp/e2e_agent.log 2>&1 &
AGENT_PID=$!

cleanup() { kill $LLAMA_PID $ORQ_PID $AGENT_PID 2>/dev/null; }
trap cleanup EXIT

# 4) espera o llama-server ficar saudável
for i in $(seq 1 60); do
    if curl -sf "http://127.0.0.1:$((PORT+100))/health" > /dev/null 2>&1; then break; fi
    sleep 1
done
curl -sf "http://127.0.0.1:$((PORT+100))/health" > /dev/null || { echo "✗ llama-server não subiu"; exit 1; }
echo "[e2e] llama-server saudável"

sleep 3  # orchestrator + agent assinam a malha

# 5) submete uma task via HTTP (T-401)
RESP=$(curl -sf -X POST "http://127.0.0.1:$PORT/api/v1/chat/completions" \
    -H 'Content-Type: application/json' \
    -d '{"model":"phi4-mini","messages":[{"role":"user","content":"Reply with exactly the word: OK"}],"max_tokens":16,"temperature":0.0,"stream":true}')
echo "[e2e] POST resposta: $RESP"
TASK_ID=$(echo "$RESP" | python3 -c "import sys,json; print(json.load(sys.stdin)['task_id'])")
echo "[e2e] task_id=$TASK_ID"

# 6) observa a malha até DONE com conteúdo real (via monitor Rust? Python dds_backend)
python3 - "$DOMAIN" "$TASK_ID" <<'EOF'
import os, sys, time
sys.path.insert(0, "/run/host/var/mnt/HD1TB/tese/src/orchestrator")
from dds_backend.dds_data_space import DDSDataSpace
from orchestrator.models import TaskStatus

domain, task_id = int(sys.argv[1]), sys.argv[2]
ds = DDSDataSpace(domain_id=domain)
done = None
start = time.time()
while time.time() - start < 90:
    t = ds.read_task(task_id)
    if t and t.status in (TaskStatus.DONE, TaskStatus.FAILED):
        done = t
        break
    time.sleep(0.5)

if done is None:
    print("[e2e] ✗ timeout esperando a task terminar"); sys.exit(1)
if done.status == TaskStatus.FAILED:
    print(f"[e2e] ✗ task FALHOU: {done.finish_reason}"); sys.exit(1)

outputs = ds.read_outputs(task_id)
content = "".join(o.content for o in outputs)
print(f"[e2e] task DONE por {done.assigned_agent!r}: content={content!r} ({len(outputs)} chunks)")

assert done.assigned_agent == "agent-rust-e2e", f"executada por {done.assigned_agent!r}"
assert content.strip(), "conteúdo vazio"
agents = ds.all_agents()
assert any(a.agent_id == "agent-rust-e2e" for a in agents), "agente ausente no registry"
print("[e2e] ✓ SUCESSO: task executada end-to-end com inferência real; agente no registry")
EOF
RC=$?

echo "=== logs ==="
grep -E "qos_decision|task concluída" /tmp/e2e_orq.log /tmp/e2e_agent.log 2>/dev/null | head -6
exit $RC
