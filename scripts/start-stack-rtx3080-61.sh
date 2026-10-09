#!/bin/sh
# Pilha completa no servidor RTX 3080 (192.168.1.61), domínio 170:
# orchestrator + policy-engine + mcp-gateway + agent (engine dds → llama-server).
# Pré-requisitos já instalados no host: binários em ~/dds-llm-rust/ (musl),
# llama-server com --enable-dds --dds-domain 170 (ver /tmp/llama-dds.log),
# studio-noded (unit systemd), XML de peers em ~/.config/cyclonedds/.
# Réplica do script ~/dds-llm-rust/start-stack.sh criado na VM em 2026-10-06.
set -e
cd "$HOME/dds-llm-rust"
mkdir -p logs sandbox
export CYCLONEDDS_URI="file://$HOME/.config/cyclonedds/studio-presence.xml"

pkill -x orchestrator 2>/dev/null || true; sleep 0.3
nohup ./orchestrator --port 8080 --dds-domain 170 --qos-manager static >> logs/orchestrator.log 2>&1 &

pkill -x policy-engine 2>/dev/null || true; sleep 0.3
nohup ./policy-engine --dds-domain 170 --policy-file "$HOME/dds-llm-rust/policies.json" >> logs/policy-engine.log 2>&1 &

pkill -x mcp-gateway 2>/dev/null || true; sleep 0.3
echo "funciona via gateway" > sandbox/prova.txt
nohup ./mcp-gateway --dds-domain 170 --filesystem-root "$HOME/dds-llm-rust/sandbox" >> logs/mcp-gateway.log 2>&1 &

pkill -x agent 2>/dev/null || true; sleep 0.3
nohup ./agent --agent-id agent-rtx3080-01 --engine dds --dds-domain 170 --slots 4 >> logs/agent-rtx.log 2>&1 &
