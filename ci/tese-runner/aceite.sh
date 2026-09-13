#!/bin/bash
# Aceite reproduzível da imagem do runner da tese (ci/tese-runner/aceite.sh).
#
# Duas fases independentes:
#   build    — valida compilação/testes de uma revisão Git dentro da imagem
#              (entrypoint substituído explicitamente; NADA registra no Gitea)
#   bootstrap— valida o comportamento do entrypoint em ambiente descartável
#              (falha fechada sem GITEA_*; pulo de registro com .runner
#              existente) — SEM Gitea real.
#
# Parâmetros explícitos (env): GIT_REV (obrigatório), IMAGE (default
# tese-runner:0.2.1), WORK (default /tmp/tese-aceite), MEM/CPUS (limites),
# CLAIM_REPS (repetições do teste de claim, default 5, estado novo cada).
#
# Preserva códigos de saída; encerra somente o que criou; não remove locks
# Cargo com compilação ativa; distingue local (.51) e remoto (operadora).
set -uo pipefail

GIT_REV="${GIT_REV:?uso: GIT_REV=<revisão> [IMAGE=tese-runner:0.2.1] ./aceite.sh {build|bootstrap|all}}"
IMAGE="${IMAGE:-tese-runner:0.2.1}"
WORK="${WORK:-/tmp/tese-aceite}"
MEM="${MEM:-8g}"
CPUS="${CPUS:-4}"
CLAIM_REPS="${CLAIM_REPS:-5}"
PHASE="${1:?fase: build|bootstrap|all}"

fail() { echo "ACEITE_FALHOU: $*" >&2; exit 1; }

# Guarda destrutiva: WORK é alvo de rm -rf. Recusar caminhos vazios,
# raiz, áreas de usuário/sistema e qualquer coisa fora da área exclusiva
# do teste (/tmp/tese-* ou /var/tmp/tese-*), sem ".." e com profundidade
# mínima. Nenhuma limpeza destrutiva sobre caminho arbitrário de env.
validate_work() {
  local w="${1:?}"
  case "$w" in
    /tmp/tese-*|/var/tmp/tese-*) ;;
    *) return 1 ;;
  esac
  case "$w" in
    *..*|"/tmp"|"") return 1 ;;
  esac
  [ "$w" != "/" ] || return 1
  return 0
}
validate_work "$WORK" || fail "WORK recusado pela guarda destrutiva: '$WORK' (use /tmp/tese-*)"

# Este script roda NO .51 (onde a imagem é construída/testada). O checkout
# da revisão vem da operadora via git archive sobre SSH — o .51 não tem o
# repositório. Variante local: LOCAL_TARBALL=<tar.gz> para pular o archive.

run_build() {
  echo "=== ACEITE build: rev=$GIT_REV image=$IMAGE work=$WORK mem=$MEM cpus=$CPUS ==="
  rm -rf "$WORK" || fail "rm -rf $WORK"
  [ ! -e "$WORK" ] || fail "$WORK persistiu apos remocao"
  mkdir -p "$WORK" || fail "mkdir $WORK"
  if [ -n "${LOCAL_TARBALL:-}" ]; then
    tar -xzf "$LOCAL_TARBALL" -C "$WORK" || fail "extrair $LOCAL_TARBALL"
    [ -f "$WORK/Cargo.toml" ] || fail "extracao sem Cargo.toml (tarball velho/vazio?)"
  else
    OPERADORA="${OPERADORA:-mzet@localhost}"
    ssh "$OPERADORA" "cd ~/projetos/tese/src/rust && git archive $GIT_REV" | tar -x -C "$WORK" \
      || fail "git archive de $GIT_REV"
  fi
  # Rodar este script como root no .51 (infra): o chown precisa valer
  # para o uid 1001 dos contêineres — sem sudo best-effort silencioso.
  chown -R 1001:1001 "$WORK" || fail "chown 1001 em $WORK (rode como root)"

  # Container ÚNICO para toda a fase build (CARGO_HOME aquecido no volume;
  # imagem nova a cada fase — sem pacotes de sessão anterior).
  docker run --rm --entrypoint sh --memory "$MEM" --cpus "$CPUS" \
    -v "$WORK":/work -w /work -e CARGO_HOME=/work/.cargo-home \
    "$IMAGE" -exc '
      echo "== toolchain =="; rustc --version; cargo --version; cmake --version | head -1; node --version
      echo "== uid =="; id
      echo "== fmt =="; cargo fmt --all -- --check
      echo "== clippy =="; cargo clippy --workspace --all-targets --all-features --locked -- -D warnings
      echo "== fetch =="; cargo fetch --locked
      echo "== test (no-fail-fast diagnostico) =="
      cargo test --workspace --all-features --locked --no-fail-fast -- --test-threads=1 \
        || { echo "SUITE_COM_FALHAS"; exit 7; }
      echo "== build release =="
      cargo build --workspace --all-features --locked --offline --release
      ls -la target/release | grep -vE "^total|^\." | head -12
    ' 2>&1 | tee "$WORK/aceite-build.log"
  local rc=${PIPESTATUS[0]}
  [ "$rc" -eq 0 ] || fail "fase build exit=$rc (ver $WORK/aceite-build.log)"
  echo "ACEITE_BUILD_OK"

  # Repetições do claim: estado novo a cada (container novo), N predefinido.
  echo "=== claim x$CLAIM_REPS (estado novo cada) ==="
  local i
  for i in $(seq 1 "$CLAIM_REPS"); do
    docker run --rm --entrypoint sh --memory "$MEM" --cpus "$CPUS" \
      -v "$WORK":/work -w /work -e CARGO_HOME=/work/.cargo-home \
      "$IMAGE" -c "cargo test --locked --offline --features dds -p mcp-gateway --test claim -- --test-threads=1 --exact claim_prevents_duplicate_execution_with_two_gateways" \
      || fail "claim repetição $i/$CLAIM_REPS falhou"
    echo "claim $i/$CLAIM_REPS OK"
  done
  echo "ACEITE_CLAIM_OK"
}

run_bootstrap() {
  echo "=== ACEITE bootstrap: entrypoint em ambiente descartável (SEM Gitea real) ==="
  local BS="$WORK/bootstrap"; mkdir -p "$BS"

  # Stub de act_runner: registra chamadas/args (não sensíveis) em
  # /stub.log e simula o contrato do entrypoint (register cria .runner;
  # daemon fica em loop até o timeout do verificador). Um {} em .runner
  # apenas exercita o ramo do script — credenciais/daemon reais são
  # comprovados no piloto, não aqui.
  local STUB='#!/bin/sh
log() { echo "STUB $*" >> /tmp/stub.log; }
if [ "$1" = "register" ]; then
  log "register $*"
  if [ "${STUB_REGISTER_FAILS:-}" = "1" ]; then exit 3; fi
  echo "{}" > /data/.runner
  exit 0
fi
if [ "$1" = "daemon" ]; then
  log "daemon $*"
  [ -f /data/.runner ] || { log "daemon-sem-estado"; exit 4; }
  while :; do sleep 1; done
fi
exit 9'

  # 1) Sem GITEA_*: falha fechada, sem efeito e SEM chamar o runner.
  OUT=$(docker run --rm --entrypoint sh -e STUB="$STUB" "$IMAGE" -c '
      printf "%s\n" "$STUB" > /tmp/act_runner && chmod +x /tmp/act_runner && export PATH=/tmp:$PATH
      mkdir -p /data; cd /data; rm -f /tmp/stub.log
      set +e; timeout 5 /usr/local/bin/entrypoint.sh 2>&1; rc=$?; set -e
      echo "ENTRY_RC=$rc"; test ! -f /tmp/stub.log && echo "NO_RUNNER_CALL" || cat /tmp/stub.log'       2>/dev/null || true)
  echo "$OUT" | head -4 | tee "$BS/failclosed.log"
  echo "$OUT" | grep -q "GITEA_INSTANCE_URL ausente" || fail "fail-closed: mensagem esperada ausente"
  echo "$OUT" | grep -q "NO_RUNNER_CALL" || fail "fail-closed: entrypoint chamou o runner sem config"

  # 2) Primeiro boot: registra UMA vez (stub), cria estado e encaminha ao
  #    daemon com a config anunciada.
  OUT=$(docker run --rm --entrypoint sh -e STUB="$STUB" "$IMAGE" -c '
      printf "%s\n" "$STUB" > /tmp/act_runner && chmod +x /tmp/act_runner && export PATH=/tmp:$PATH
      mkdir -p /data; cd /data; rm -f /tmp/stub.log
      export GITEA_INSTANCE_URL=http://127.0.0.1:9 GITEA_RUNNER_REGISTRATION_TOKEN=dummy
      export GITEA_RUNNER_NAME=aceite GITEA_RUNNER_LABELS=aceite
      set +e; timeout 4 /usr/local/bin/entrypoint.sh; rc=$?; set -e
      echo "ENTRY_RC=$rc"; cat /tmp/stub.log; test -f /data/.runner && echo STATE_CREATED'       2>/dev/null || true)
  echo "$OUT" | head -6 | tee "$BS/firstboot.log"
  echo "$OUT" | grep -q "STUB register register --no-interactive --instance http://127.0.0.1:9 --token "     || fail "first boot: register nao chamado com args esperados"
  echo "$OUT" | grep -q -- "-- --labels aceite" || echo "$OUT" | grep -q -- "--labels aceite" || fail "first boot: labels ausentes no register"
  echo "$OUT" | grep -q "STUB daemon daemon --config /config.yaml"     || fail "first boot: daemon nao encaminhado com a config anunciada"
  echo "$OUT" | grep -q STATE_CREATED || fail "first boot: estado .runner nao criado"

  # 3) Boot seguinte com estado existente: NAO registra; daemon direto.
  OUT=$(docker run --rm --entrypoint sh -e STUB="$STUB" "$IMAGE" -c '
      printf "%s\n" "$STUB" > /tmp/act_runner && chmod +x /tmp/act_runner && export PATH=/tmp:$PATH
      mkdir -p /data; cd /data; echo "{}" > /data/.runner; rm -f /tmp/stub.log
      export GITEA_INSTANCE_URL=http://127.0.0.1:9 GITEA_RUNNER_REGISTRATION_TOKEN=dummy
      export GITEA_RUNNER_NAME=aceite GITEA_RUNNER_LABELS=aceite
      set +e; timeout 4 /usr/local/bin/entrypoint.sh; rc=$?; set -e
      echo "ENTRY_RC=$rc"; cat /tmp/stub.log'       2>/dev/null || true)
  echo "$OUT" | head -4 | tee "$BS/secondboot.log"
  echo "$OUT" | grep -q "STUB register register" && fail "second boot: registrou de novo (duplicacao)"
  echo "$OUT" | grep -q "STUB daemon daemon --config /config.yaml"     || fail "second boot: daemon nao encaminhado"

  # 4) NEGATIVO do verificador: registro que falha (stub exit 3) deve
  #    terminar o entrypoint com código != 0 — e o próprio aceite
  #    demonstra que falha de verificação => exit != 0 do aceite.
  OUT=$(docker run --rm --entrypoint sh -e STUB="$STUB" "$IMAGE" -c '
      printf "%s\n" "$STUB" > /tmp/act_runner && chmod +x /tmp/act_runner && export PATH=/tmp:$PATH
      mkdir -p /data; cd /data; rm -f /tmp/stub.log
      export GITEA_INSTANCE_URL=http://127.0.0.1:9 GITEA_RUNNER_REGISTRATION_TOKEN=dummy
      export GITEA_RUNNER_NAME=aceite GITEA_RUNNER_LABELS=aceite STUB_REGISTER_FAILS=1
      set +e; timeout 4 /usr/local/bin/entrypoint.sh; rc=$?; set -e
      echo "ENTRY_RC=$rc"'       2>/dev/null || true)
  echo "$OUT" | head -2 | tee "$BS/negative.log"
  RC_NEG=$(echo "$OUT" | grep -oE "ENTRY_RC=[0-9]+" | cut -d= -f2)
  [ -n "$RC_NEG" ] && [ "$RC_NEG" -ne 0 ]     || fail "negativo: registro falhando deveria encerrar !=0 (obtido: '$RC_NEG')"

  # 5) uid 1001 sem preparo (captura em variável; grep -q + SIGPIPE).
  UIDOUT=$(docker run --rm --entrypoint sh "$IMAGE" -c 'id; rustc --version')
  echo "$UIDOUT" | grep -q "uid=1001" || fail "imagem não executa como uid 1001 (saída: $UIDOUT)"
  echo "ACEITE_BOOTSTRAP_OK"
}

case "$PHASE" in
  build) run_build ;;
  bootstrap) run_bootstrap ;;
  all) run_bootstrap; run_build ;;
  *) fail "fase inválida: $PHASE" ;;
esac
echo "ACEITE_OK ($PHASE)"
