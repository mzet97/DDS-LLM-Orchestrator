#!/bin/sh
# Bootstrap explícito do act_runner (registro + daemon), sem herdar fluxo
# de outra imagem e sem duplicar runners a cada reinício.
#
# - Primeiro boot (sem /data/.runner): registra UMA vez com o token de
#   registro (env GITEA_RUNNER_REGISTRATION_TOKEN, vindo do Secret) e
#   guarda o estado em /data/.runner.
# - Boots seguintes: pula o registro e executa o daemon com --config.
# - /data é emptyDir: se o volume for recriado, um NOVO registro ocorre e
#   o runner antigo (mesmo nome) deve ser removido no Gitea — registrado
#   no log de ativação, nunca silencioso.
set -eu

INSTANCE="${GITEA_INSTANCE_URL:?GITEA_INSTANCE_URL ausente}"
CONFIG="${RUNNER_CONFIG_FILE:-/config.yaml}"

if [ ! -f /data/.runner ]; then
  echo "entrypoint: registrando runner em $INSTANCE"
  act_runner register \
    --no-interactive \
    --instance "$INSTANCE" \
    --token "${GITEA_RUNNER_REGISTRATION_TOKEN:?token ausente}" \
    --name "${GITEA_RUNNER_NAME:?nome ausente}" \
    --labels "${GITEA_RUNNER_LABELS:?labels ausentes}"
else
  echo "entrypoint: registro existente, pulando (sem duplicar)"
fi

exec act_runner daemon --config "$CONFIG"
