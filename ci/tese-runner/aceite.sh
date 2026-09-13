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

# Este script roda NO .51 (onde a imagem é construída/testada). O checkout
# da revisão vem da operadora via git archive sobre SSH — o .51 não tem o
# repositório. Variante local: LOCAL_TARBALL=<tar.gz> para pular o archive.
fail() { echo "ACEITE_FALHOU: $*" >&2; exit 1; }

run_build() {
  echo "=== ACEITE build: rev=$GIT_REV image=$IMAGE work=$WORK mem=$MEM cpus=$CPUS ==="
  rm -rf "$WORK"; mkdir -p "$WORK"
  if [ -n "${LOCAL_TARBALL:-}" ]; then
    tar -xzf "$LOCAL_TARBALL" -C "$WORK"
  else
    OPERADORA="${OPERADORA:-mzet@localhost}"
    ssh "$OPERADORA" "cd ~/projetos/tese/src/rust && git archive $GIT_REV" | tar -x -C "$WORK" \
      || fail "git archive de $GIT_REV"
  fi
  chown -R 1001:1001 "$WORK" 2>/dev/null || sudo chown -R 1001:1001 "$WORK"

  # Container ÚNICO para toda a fase build (CARGO_HOME aquecido no volume;
  # imagem nova a cada fase — sem pacotes de sessão anterior).
  docker run --rm --entrypoint sh --memory "$MEM" --cpus "$CPUS" \
    -v "$WORK":/work -w /work -e CARGO_HOME=/work/.cargo-home \
    "$IMAGE" /bin/sh -exc '
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
  # 1) Sem GITEA_*: falha fechada, sem efeito.
  OUT=$(docker run --rm "$IMAGE" 2>&1); RC=$?
  echo "$OUT" | head -3
  [ $RC -ne 0 ] || fail "entrypoint sem GITEA_* deveria falhar (rc=$RC)"
  echo "bootstrap fail-closed OK (rc=$RC)"

  # 2) Com GITEA_* apontando para endereço INEXISTENTE: tenta registro,
  #    falha por rede — nunca toca o Gitea real; e com .runner existente,
  #    pula o registro e segue para o daemon.
  docker run --rm --entrypoint sh "$IMAGE" -c '
      mkdir -p /data && echo "{}" > /data/.runner
      GITEA_INSTANCE_URL=http://127.0.0.1:9 \
      GITEA_RUNNER_REGISTRATION_TOKEN=dummy-nao-usado \
      GITEA_RUNNER_NAME=teste GITEA_RUNNER_LABELS=teste \
      timeout 5 /usr/local/bin/entrypoint.sh 2>&1 | head -4
    ' 2>&1 | tee "$WORK/../aceite-bootstrap.log" | head -6
  echo "bootstrap .runner-existente/pula-registro executado (ver log)"

  # 3) Executa como UID 1001 direto (imagem pronta não exige preparo).
  docker run --rm --entrypoint sh "$IMAGE" -c 'id; rustc --version' \
    | grep -q "uid=1001" || fail "imagem não executa como uid 1001"
  echo "ACEITE_BOOTSTRAP_OK"
}

case "$PHASE" in
  build) run_build ;;
  bootstrap) run_bootstrap ;;
  all) run_bootstrap; run_build ;;
  *) fail "fase inválida: $PHASE" ;;
esac
echo "ACEITE_OK ($PHASE)"
