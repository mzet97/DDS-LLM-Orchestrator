#!/usr/bin/env bash
# T-840-02: deploy idempotente do studio-noded em um host remoto.
#
# Uso:
#   scripts/deploy-studio-node.sh <user@host> [servicos_csv] [token]
#
# - Build musl estático local (roda em qualquer glibc; validado nos hosts
#   Ubuntu 24.04/glibc 2.39 contra glibc 2.43 local — T-840, audit 2026-10-05).
# - Instala binário em ~/dds-llm-rust/ + unit systemd --user + env com
#   STUDIO_NODE_BIND=0.0.0.0 e STUDIO_NODE_TOKEN (>=16 chars, obrigatório
#   para bind LAN — T-840-01).
# - Token: se não passado, gera um e salva em ~/.config/studio/hosts/<host>.token
#   (SÓ na máquina local; nunca no repositório — segredos não versionam).
# - Mata processos órfãos de studio-noded (era sem unit), daemon-reload,
#   enable --now, linger best-effort e health check com o token.
set -euo pipefail

# Senha SSH NUNCA tem fallback neste script (segredos não versionam —
# anti-pattern do repo). Exporte antes: export SSH_PASS='...'
SSH_PASS="${SSH_PASS:?export SSH_PASS antes de executar deploy-studio-node.sh}"

TARGET="${1:?uso: deploy-studio-node.sh <user@host> [servicos_csv] [token]}"
SERVICES="${2:-dds-agent}"
ROOT="$(cd "$(dirname "$0")/.." && pwd)"
PORT="${STUDIO_NODE_PORT:-4317}"

TOKEN="${3:-}"
if [[ -z "$TOKEN" ]]; then
  HOSTKEY="${TARGET#*@}"
  TOKEN_DIR="$HOME/.config/studio/hosts"
  mkdir -p "$TOKEN_DIR"
  TOKEN_FILE="$TOKEN_DIR/$HOSTKEY.token"
  if [[ -f "$TOKEN_FILE" ]]; then
    TOKEN="$(cat "$TOKEN_FILE")"
  else
    TOKEN="$(head -c 24 /dev/urandom | base64 | tr -d "=+/" | head -c 32)"
    umask 077; printf '%s' "$TOKEN" > "$TOKEN_FILE"; umask 022
  fi
fi

echo "== [1/5] build musl estático (feature dds — presença Studio.NodePresence, T-890) =="
# Cross-build musl + CycloneDDS C vendored (ladder T-890-02 passo 1 — validado
# 2026-10-06): requer toolchain musl (musl.cc em ~/.local/musl-cross), snapshot
# ABI do sys crate para o target (abi/x86_64-unknown-linux-musl.rs — gerado com
# o probe estático; layouts glibc≡musl verificados por diff) e CC apontando o
# musl-gcc NA MESMA invocação (o cmake só honra CC na 1ª configuração).
MUSL_CC="${MUSL_CC:-$HOME/.local/musl-cross/x86_64-linux-musl-cross/bin/x86_64-linux-musl-gcc}"
[[ -x "$MUSL_CC" ]] || { echo "toolchain musl ausente: $MUSL_CC"; exit 1; }
MUSL_DIR="${CARGO_TARGET_DIR:-$HOME/.cache/tese-rust-target-musl-dds}"
MUSL_BIN_DIR="$(dirname "$MUSL_CC")"
CARGO_TARGET_DIR="$MUSL_DIR" PATH="$MUSL_BIN_DIR:$PATH" \
  CC="$MUSL_CC" AR="$(dirname "$MUSL_CC")/x86_64-linux-musl-ar" \
  CC_x86_64_unknown_linux_musl="$MUSL_CC" \
  CARGO_TARGET_X86_64_UNKNOWN_LINUX_MUSL_LINKER="$MUSL_CC" \
  CYCLONEDDS_STATIC=1 \
  RUSTFLAGS="-C target-feature=+crt-static" \
  cargo build --release --target x86_64-unknown-linux-musl -p studio-node --features dds
BIN="$MUSL_DIR/x86_64-unknown-linux-musl/release/studio-noded"
[[ -x "$BIN" ]] || { echo "binário não encontrado: $BIN"; exit 1; }

echo "== [2/5] copiando para $TARGET =="
sshpass -p "$SSH_PASS" ssh "$TARGET" 'mkdir -p ~/dds-llm-rust ~/.config/systemd/user ~/.config/studio-noded'
sshpass -p "$SSH_PASS" scp -q "$BIN" "$TARGET:dds-llm-rust/studio-noded.new"
sshpass -p "$SSH_PASS" scp -q "$ROOT/packaging/systemd/studio-noded.service" "$TARGET:.config/systemd/user/studio-noded.service"

echo "== [3/5] instalando unit/env e reiniciando =="
HOSTKEY="${TARGET#*@}"
HOSTIP="$HOSTKEY"
sshpass -p "$SSH_PASS" ssh "$TARGET" "
  set -e
  # Órfãos da era sem unit (bind antigo): nome exato, sem casar a sessão ssh.
  pkill -x studio-noded 2>/dev/null || true
  sleep 0.3
  chmod +x ~/dds-llm-rust/studio-noded.new
  mv ~/dds-llm-rust/studio-noded.new ~/dds-llm-rust/studio-noded
  # Descoberta unicast do lab (T-890-02; validada na fase 870): sem xmlns —
  # o CycloneDDS vendido IGNORA config com xmlns — id any, multicast off,
  # peers unicast dos hosts do laboratório.
  mkdir -p ~/.config/cyclonedds
  cat > ~/.config/cyclonedds/studio-presence.xml <<XML
<?xml version=\"1.0\" encoding=\"utf-8\"?>
<CycloneDDS>
  <Domain id=\"any\">
    <General>
      <AllowMulticast>false</AllowMulticast>
    </General>
    <Discovery>
      <ParticipantIndex>auto</ParticipantIndex>
      <MaxAutoParticipantIndex>50</MaxAutoParticipantIndex>
      <Peers>
        <Peer address=\"192.168.1.61\" />
        <Peer address=\"192.168.1.62\" />
        <Peer address=\"192.168.1.63\" />
        <Peer address=\"192.168.1.64\" />
        <Peer address=\"127.0.0.1\" />
      </Peers>
    </Discovery>
    <Internal>
      <WriterLingerDuration>0ms</WriterLingerDuration>
    </Internal>
  </Domain>
</CycloneDDS>
XML
  umask 077
  # \$HOME expande NO HOST REMOTO (heredoc sem aspas) — systemd EnvironmentFile
  # NÃO expanda variáveis (lição 830: specifiers %h também não funcionam lá).
  cat > ~/.config/studio-noded/env <<ENVFILE
STUDIO_NODE_BIND=0.0.0.0
STUDIO_NODE_PORT=$PORT
STUDIO_NODE_SERVICES=$SERVICES
STUDIO_NODE_TOKEN=$TOKEN
STUDIO_NODE_DDS_DOMAIN=${STUDIO_NODE_DDS_DOMAIN:-170}
STUDIO_NODE_PUBLIC_URL=http://$HOSTIP:$PORT
CYCLONEDDS_URI=file://\$HOME/.config/cyclonedds/studio-presence.xml
ENVFILE
  chmod 600 ~/.config/studio-noded/env
  systemctl --user daemon-reload
  systemctl --user enable --now studio-noded.service 2>/dev/null || systemctl --user restart studio-noded.service
  loginctl enable-linger 2>/dev/null || echo 'aviso: linger não ativado (nó morre no logout total)'
"

echo "== [4/5] health check =="
HOSTKEY="${TARGET#*@}"
for _ in $(seq 1 10); do
  if curl -fsS --max-time 3 "http://$HOSTKEY:$PORT/version" | grep -q '"major":1'; then
    echo "nó ativo em http://$HOSTKEY:$PORT (protocolo 1.x, token exigido nas rotas administrativas)"
    echo "token salvo localmente em: ${TOKEN_FILE:-stdin}"
    exit 0
  fi
  sleep 1
done
echo "FALHA: nó não respondeu em http://$HOSTKEY:$PORT/version"
exit 1
