#!/bin/bash
# Servidor SSH descartável para validação da GUI do Studio em loopback.
#
# Uso exclusivo: roteiro de validação visual do painel SSH
# (scripts/ROTEIRO-GUI-SSH.md). Nenhuma VM, nenhum
# host real, nenhum authorized_keys fora daqui.
#
#   ./studio-ssh-descartavel.sh start   # sobe em 127.0.0.1:porta livre
#   ./studio-ssh-descartavel.sh add-pub <arquivo.pub>
#   ./studio-ssh-descartavel.sh status
#   ./studio-ssh-descartavel.sh stop
#
# Limites: só loopback, só pubkey (sem senha/PAM/root), um sshd por
# diretório de estado; `stop` mata somente o PID registrado.
set -euo pipefail

STATE="${STUDIO_SSH_DIR:-/tmp/studio-ssh-descartavel}"
mkdir -p "$STATE"
HOSTKEY="$STATE/ssh_host_ed25519_key"
AUTHKEYS="$STATE/authorized_keys"
PIDFILE="$STATE/sshd.pid"
PORTFILE="$STATE/port"

case "${1:-}" in
  start)
    test ! -f "$PIDFILE" || { echo "já em execução (ver status)"; exit 1; }
    test -f "$HOSTKEY" || ssh-keygen -q -t ed25519 -N "" -f "$HOSTKEY"
    touch "$AUTHKEYS"; chmod 600 "$AUTHKEYS"
    PORT="$(python3 -c 'import socket;s=socket.socket();s.bind(("127.0.0.1",0));print(s.getsockname()[1])')"
    echo "$PORT" > "$PORTFILE"
    /usr/bin/sshd -f /dev/null \
      -o "ListenAddress 127.0.0.1:$PORT" \
      -o "HostKey $HOSTKEY" \
      -o "AuthorizedKeysFile $AUTHKEYS" \
      -o "PasswordAuthentication no" \
      -o "KbdInteractiveAuthentication no" \
      -o "UsePAM no" \
      -o "PermitRootLogin no" \
      -o "StrictModes no" \
      -o "PidFile $PIDFILE" \
      -E "$STATE/sshd.log"
    echo "sshd descartável em 127.0.0.1:$PORT (impressão abaixo)"
    ssh-keygen -l -f "$HOSTKEY.pub"
    ;;
  add-pub)
    test -f "${2:?informe o .pub gerado pela GUI}" || exit 1
    grep -qxF -e "$(head -c 4096 "$2")" "$AUTHKEYS" 2>/dev/null \
      && echo "já cadastrada" || cat "$2" >> "$AUTHKEYS"
    echo "cadastrada(s) em $AUTHKEYS: $(wc -l < "$AUTHKEYS")"
    ;;
  status)
    if test -f "$PIDFILE" && kill -0 "$(cat "$PIDFILE")" 2>/dev/null; then
      echo "ativo pid=$(cat "$PIDFILE") porta=$(cat "$PORTFILE")"
    else
      echo "parado"; exit 1
    fi
    ;;
  stop)
    if test -f "$PIDFILE"; then
      PID="$(cat "$PIDFILE")"
      PORT="$(cat "$PORTFILE" 2>/dev/null || echo '?')"
      kill "$PID" 2>/dev/null || true
      for _ in 1 2 3 4 5; do
        kill -0 "$PID" 2>/dev/null || break
        sleep 0.3
      done
      if kill -0 "$PID" 2>/dev/null; then
        echo "ERRO: pid $PID ainda vivo — servidor NÃO encerrado"
        exit 1
      fi
      if ss -ltn 2>/dev/null | grep -q "127.0.0.1:$PORT "; then
        echo "ERRO: porta $PORT ainda com listener"
        exit 1
      fi
      rm -f "$PIDFILE"
      echo "parado e confirmado: pid $PID inexistente, porta $PORT sem listener"
    else
      echo "parado (sem pidfile)"
    fi
    ;;
  *)
    echo "uso: $0 {start|add-pub <f.pub>|status|stop}" >&2; exit 2
    ;;
esac
