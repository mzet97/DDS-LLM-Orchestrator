# Registro de validação — CI/CD da tese (sem segredos)
#
# Caminhos e procedimentos já executados, para não reabrir perguntas.

## CA pública `local-root-ca`
- Origem: Secret `local-root-ca` (ns `cert-manager`, cluster .51),
  campo `tls.crt` — SOMENTE o certificado público; a chave privada nunca
  foi tocada.
- Impressão SHA-256 (conferida via openssl no cluster E via python na
  operadora — idênticas):
  `A2:CB:24:84:56:4B:72:DC:65:64:E5:AD:F9:C6:F9:D0:8A:47:D1:DD:84:0A:70:28:55:5B:39:80:3F:E9:9A:5C`
- Destino na operadora: `~/.config/dds-orchestrator/pki/local-root-ca.crt`
  (dir 0700, arquivo 0600; nunca sobrescrever certificado diferente sem
  conferir).
- Uso: confiança explícita por ferramenta (`curl --cacert`, `oras
  --ca-file`). Sem `-k`, sem instalação global no F0.
- Prova: 200 em gitea/harbor/argocd `.home.arpa` com `--cacert`.

## Cluster (somente leitura)
- Acesso: `k8s1@192.168.1.51` (kubectl no host; kubeconfig administrativo
  NÃO copiado para operadora/runner/GUI).
- Runner existente: `ci-runners/gitea-act-runner` Running, v0.2.12,
  labels `ubuntu-latest, ubuntu-22.04`, `capacity: 2`, SEM toolchain Rust.
- ArgoCD: `root` Synced/Healthy de `gitea_admin/gitops.git`,
  `gitops/bootstrap` — NÃO é o checkout `k8s`; não alterar.
- Gitea 1.26.1; erro de banco (`pq ... shutting down`) em 12/09 + 27
  restarts: ocorrências a investigar, sem causa raiz afirmada.
- containerd: mirror `harbor.harbor.svc.cluster.local` → HTTP
  `10.43.105.175` (pulls internos OK sem TLS; CA só para acesso externo).

## Imagem `tese-runner:0.1.0` (bootstrap, só local no .51, SEM push)
- Digest: `sha256:7b542446f6a54217098576759d293c3c4bb0e2356098339a701bd8fbc28386cc`
- Smoke: usuário `runner`, rustc/cargo 1.95.0, node v24.21.0,
  SEM docker socket, SEM chaves SSH.
- act_runner 0.2.12 com SHA verificado contra o publicado:
  `0b6d1ca5487e737bc67ecb440997dff412dd63d65b291eddcb69aec9cb61ebbf`

## Canário local do daemon (revisão publicada 409c3a5, 2026-09-12)
- Revisão: `409c3a5` (branch `studio/phase-800-node`, árvore limpa,
  scripts com bit executável). Reprodução em checkout separado
  (`git clone` + `checkout 409c3a5`):
  `cargo build --locked --release -p studio-node` (15.9s) +
  `./ansible/tests/canary_local.sh` → `CANARIO_EXIT=0`,
  `CANÁRIO OK: deploy, reinício, validação, falha e rollback demonstrados`.
- Binários (mesmo commit-fonte, perfis distintos):
  v1 release `91afef86c968bf4d313e4e210097f34d602ce952d4ff08825d1c4cbf91094c3a`;
  v2 debug `f4b9872bce1f680d0d1ae63b02dbe3fa42b520100ff5e049539fb9c8d20539e0`.
  Conclusão limitada ao procedimento de substituição/reinício.
- NÃO exercitados: playbooks Ansible, artefato OCI, systemd, SSH, GUI, DDS.
- Validador: `test_validate_db.py` 8/8 (1 positivo + 6 negativos + flag).
- Bloqueios externos (sem tocar em serviços): SSH `k8s1@192.168.1.51`
  `Permission denied` com as chaves locais (porta 22 aberta); Harbor
  `GET /v2/` responde 401 + `WWW-Authenticate: Bearer` (desafio normal,
  credencial robot ainda não disponibilizada).
  → RESOLVIDOS em 2026-09-12 (seções abaixo).

## SSH administrativo restaurado (2026-09-12, com autorização expressa)
- Pub `homelab-secure-edge-ansible` (`SHA256:Eu2fDEXUZ93PJx1A3cOSer8VJ0mPMsWko7QiSgUXJuA`)
  cadastrada no `authorized_keys` do .51 por append; entradas
  pré-existentes preservadas (2 no total, fingerprints conferidos).
- Tentativa controlada única: `ssh -i ~/.ssh/homelab-secure-edge -o
  IdentitiesOnly=yes k8s1@192.168.1.51 'echo SSH_OK; docker --version'`
  → `SSH_OK`, Docker 29.4.0, exit 0. Host key validada (nunca
  `StrictHostKeyChecking=no`).

## Imagem `tese-runner:0.2.0` — reconciliação, testes e push (2026-09-12)
- Reconciliação: ID local no .51
  `sha256:33c1468bec5a8c5c6e0094ebb29ee7fff3ca6dc759c6d2c90416518a8149d5d1`
  == digest registrado em `ci/tese-runner/runner-deployment.yaml` —
  SEM divergência, sem rebuild.
- Smoke uid 1001: `uid=1001(runner)`; rustc/cargo 1.95.0, rustfmt 1.9.0,
  clippy 0.1.95, cmake 3.22.1, node v24.21.0, act_runner v0.2.12;
  `/data` gravável (DATA_OK).
- Superfície: SEM docker.sock; SEM kubeconfig (root e $HOME); find
  id_rsa/id_ed25519/known_hosts (maxdepth 4): nada; `entrypoint.sh`
  presente 0755; falha FECHADA sem `GITEA_*`; tentativa de registro
  apenas contra endereço inexistente (127.0.0.1:9) — Gitea real
  intocado (F3 permanece bloqueado).
- Harbor (autorizado): projeto `tese` criado PRIVADO; robot
  `robot$tese+tese-ci` — somente Pull+Push de repositório no projeto
  `tese`, duração 180d; segredo em arquivo 0600 fora de git (caminho:
  `~/.config/dds-orchestrator/secrets/harbor-tese-robot.env`).
- Infra necessária ao push TLS: CA do Harbor instalada no .51 em
  `/etc/docker/certs.d/harbor.home.arpa/ca.crt` e
  `/etc/containerd/certs.d/harbor.home.arpa/ca.crt` (+ pool do sistema);
  docker reiniciado 1x (iperf3-server religado; k3s/containerd do
  cluster intocados).
- Push: `tese-runner:0.2.0` → digest do REGISTRY
  `sha256:33c1468bec5a8c5c6e0094ebb29ee7fff3ca6dc759c6d2c90416518a8149d5d1`
  — IDÊNTICO ao ID local (provenância 1:1); pull por digest OK;
  header `docker-content-digest` confere. Manifesto fixado em
  `harbor.home.arpa/tese/tese-runner@sha256:33c1468b…9d5d1`.
- Pendente (exigem autorizações próprias): aplicar o Deployment,
  registrar/ativar o runner no Gitea (F3), executar workflow, promover
  release.

## GUI Studio — painel SSH dedicado, descartável (2026-09-12)
- Ambiente: app forçado a X11 (XWayland) para automação por xdotool
  ( Wayland/KDE sem injetor de entrada; uinput inacessível neste kernel:
  /dev/uinput nobody:nobody, chown negado). Janela "DDS Orchestrator
  Studio" real, controles reais.
- Fluxo: servidor descartável 127.0.0.1:50235 (impressão host
  `SHA256:jZfPU5zi0EkG8/n9gupExHuWvLHfKpRNnNhsjPOXvco`, conferida por
  ssh-keygen E pelo bloco da GUI com zoom de captura — IDÊNTICAS);
  identidade Ed25519 gerada pela GUI em `/tmp/studio-gui-ssh/gui`
  (determinística; `.pub` cadastrada via add-pub; fingerprint
  `SHA256:tjHywDqIkBv/Uy6898DLEtiPqCJLHBvoCtxfi90OCNU`); handshake real:
  3 conexões `[preauth]` no log do sshd.
- BUG 1 encontrado e CORRIGIDO: painel nunca definia
  `session.config.trust_path` → aprovação falhava com `arquivo de
  confiança ilegível em :` (caminho vazio). Fix: `views/ssh.rs` define
  `<diretório>/trust.json` (mesmo fluxo dos testes). 92/92 testes do
  crate passando (inclui aprovação→Idle→persistência).
- BUG 2 (ABERTO): após o 1º `Conectar e executar`, a janela para de
  processar eventos (hover/teclado/windowmove sem efeito; frames
  idênticos; threads sem spin visível) — o clique de aprovação não
  avalia na sessão X11 automatizada. Suspeita: laço de repaint/eventos
  (winit/egui) no caminho X11 — exige triage com mouse físico.
- Erro sem vazamento: a falha exibida (`Falhou (sem fallback): arquivo
  de confiança ilegível em …`) contém apenas texto do erro — sem senha,
  sem caminho de chave privada, sem token.

## Caminho DDS completo (domínio de teste 77, 2026-09-12)
- Participantes: `det-responder --domain 77` + `agent --agent-id
  dds-prova-77 --dds-domain 77 --engine dds` + `submit-one` com
  `PROMPT_VERSION:` de `seq_B_reviewer_v1.txt`. Diretório e processos
  próprios; malha operacional (domínio 42) intocada; tudo encerrado após.
- Resultado: `success:true`, `task_id=0c062488-…`, conteúdo do fixture
  determinístico; `resp.jsonl` com `request_id == task_id`,
  `agent_id=dds-prova-77`, `outcome:ok`.
- Correção de CLI registrada: binários diretos NÃO usam `--` separador
  (só via `cargo run --`); playbooks F3 ajustados.
