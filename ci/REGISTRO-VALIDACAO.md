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

## Mudanças de infraestrutura do .51 (2026-09-12, autorizadas)
- CA pública do Harbor instalada (fonte: CA já validada da operadora —
  certificado público, sem chave privada):
  - `/etc/docker/certs.d/harbor.home.arpa/ca.crt` (0644)
  - `/etc/containerd/certs.d/harbor.home.arpa/ca.crt` (0644)
  - `/usr/local/share/ca-certificates/local-root-ca.crt` +
    `update-ca-certificates` (confiança do sistema).
  Mantidos no host (NÃO removidos): necessários a pull/push TLS
  futuros do runner. Reversíveis removendo os arquivos +
  `update-ca-certificates --fresh`.
- `systemctl restart docker` executado 1x (o containerd store do
  Docker 29 lê `certs.d` apenas no arranque). Afetados: container
  `iperf3-server` (parado e religado em seguida — Up confirmado);
  k3s e seu containerd NÃO foram reiniciados (instância separada);
  pods do cluster seguiram Running.
- Robot `robot$tese+tese-ci` credenciado no docker config do k8s1
  (`docker login`) para o push; `docker logout` após o uso e o arquivo
  local da credencial removido.

## Compilação e testes reais na imagem publicada (2026-09-13)
- Referência executada POR DIGEST:
  `harbor.home.arpa/tese/tese-runner@sha256:33c1468b…9d5d1`.
- Código: checkout limpo de `cfe3ff2` (git archive → tar), em
  `/tmp/tese-compile-cfe3ff2` (dono uid 1001), `CARGO_HOME` próprio;
  sem credenciais, docker socket ou kubeconfig montados.
- Recursos: `--memory 8g/16g --cpus 4/16` (duas rodadas p/ discriminar
  contenção); rede do container liberada apenas para `cargo fetch`
  (crates.io) — o build final roda `--offline`.
- Comandos executados como uid 1001 (via setpriv), na ordem do
  `security.yml`:
  1. `cargo fetch --locked` ✓
  2. `cargo fmt --all -- --check` ✓
  3. `cargo clippy --workspace --all-targets --all-features --locked
     -- -D warnings` ✓ (compila CycloneDDS 11.0.0 embutido via cmake)
  4. `cargo test --workspace --all-features --locked --
     --test-threads=1` → 44 suítes `ok` + **1 FALHA**:
     - `mcp-gateway/tests/claim.rs:183`
       `claim_prevents_duplicate_execution_with_two_gateways` — 100
       tool calls não completaram no timeout de 30s (30.82s com 4
       CPUs; 30.83s com 16 CPUs). Reproduzível e INDEPENDENTE de
       recursos. Suspeita: descoberta/entrega DDS dentro do network
       namespace do container. **ABERTO p/ triage — bloqueia o uso do
       runner p/ essa suíte.**
  5. `cargo build --workspace --all-features --locked --offline
     --release` ✓ **COMPILACAO_OK** (0 erros) — artefatos uid 1001 em
     `target/release`: `agent` (4.7 MB), `context-store` (2.0 MB),
     `dds-bench` (2.0 MB) + `.fingerprint`/`deps` (652 itens).
     Nota de limpeza: um `.cargo-lock` root-órfão de um run anterior
     foi removido antes da rodada final.
- Lacuna encontrada no Dockerfile da imagem: faltam `make` e `g++`
  para o CycloneDDS embutido (só gcc foi instalado). Contornada por
  run com apt-get + `setpriv` para uid 1001 — incorporar ao Dockerfile
  na próxima iteração.

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
- Push: `tese-runner:0.2.0` → aceito pelo registry. Objetos OCI
  identificados (não são um objeto só):
  - `docker image inspect .Id` (store containerd) reporta o digest do
    **índice OCI**: `sha256:33c1468bec5a8c5c6e0094ebb29ee7fff3ca6dc759c6d2c90416518a8149d5d1`
    — é a referência fixada no manifesto (`@sha256:33c1468b…`).
  - Índice → 1 manifesto amd64
    `sha256:2e1d3232755038404ce1066342940c7e93019b9e43317ff5fcabcaf8f21c22c5`
    (oci.image.manifest.v1+json, 1813 B).
  - Manifest → config
    `sha256:1f672ebd668f8a48bd4ccce5bb4756ac00fef875040a991fea634c828165184c`
    + 8 camadas (OCI layer tar+gzip).
- Pull por digest `@sha256:33c1468b…` executado pelo **Docker do .51**
  (não valida pull pelo runtime do K3s — o Deployment ainda não foi
  aplicado); header `docker-content-digest` do registry confere com o
  índice.
- Pendente (exigem autorizações próprias): aplicar o Deployment,
  registrar/ativar o runner no Gitea (F3), executar workflow, promover
  release.

## GUI Studio — painel SSH dedicado, descartável (2026-09-12/13)
- Ambiente: app forçado a X11 (XWayland), apenas no processo de teste
  (`env -u WAYLAND_DISPLAY -u XDG_SESSION_TYPE DISPLAY=:0`), para
  automação por xdotool (Wayland/KDE sem injetor; uinput inacessível
  neste kernel: /dev/uinput nobody:nobody, chown negado). Janela real
  "DDS Orchestrator Studio" (bin `target/debug/studio` da revisão
  cfe3ff2+, `STUDIO_SSH_DEBUG=1` para instrumentação em stderr).
- Aceite CONCLUÍDO em 2026-09-13, fluxo completo pelos controles:
  servidor descartável → Gerar identidade (GUI) → add-pub → Conectar →
  bloco `Host desconhecido` com impressão IDÊNTICA à do terminal
  (conferida por zoom de captura) → **Aprovar e salvar no cofre**
  (trust.json gravado) → reconectar → **Saída: PROVA_OK** → erro
  controlado (servidor parado; `Connection refused`) sem senha, sem
  caminho de chave privada, sem token → reinício do app → reconexão
  direta (aprovação persistida recuperada, sem novo prompt).
- Correções registradas:
  - BUG 1 (CORRIGIDO, commit 27790df): painel não definia
    `trust_path` → aprovação falhava com `arquivo de confiança
    ilegível em :`. Fix: `views/ssh.rs` → `aplicar_diretorio()` define
    `<diretório>/trust.json`. Regressão nova
    (`aplicar_diretorio_configura_identidade_e_cofre`) reproduz a
    falha na versão sem fix (FAILED) e passa com o fix.
  - BUG 2 (CORRIGIDO, mesmo commit): a thread de conexão não acordava
    a UI ao concluir — `request_repaint` só existia com fase Running;
    nas transições para NeedsApproval/Done/Failed o frame não rodava
    (eventos enfileirados sem avaliação — interpretação correta do
    antigo "congelamento"). Fix: `SshSession::set_repaint_source(ctx)`
    + `ctx.request_repaint()` na thread e pós-transição (egui 0.36:
    Context Clone+Send+Sync). Log `STUDIO_SSH_DEBUG` registra
    start/fase/thread/cofre — sem senha nem conteúdo de chave.
- Correções de interpretação:
  - A identidade Ed25519 NÃO é determinística: `PrivateKey::random`
    por geração + `encrypt(passphrase)`. Teste novo
    (`identidade_aleatoria.rs`): mesma senha em diretórios novos →
    públicas DIFERENTES; identidade existente conserva a fingerprint
    ao desbloquear.
  - O painel TEM campo `usuário` (linha host/porta/usuário) — ficava
    CLIPADO na janela 800x600. Com janela 1300x720 o campo fica
    visível e foi preenchido explicitamente (`mzet`). Config inicial:
    string vazia (nada de usuário implícito).
- Instrumentação: `STUDIO_SSH_DEBUG=1` registra handler de botões,
  transições de fase (Idle/Running/NeedsApproval/Done/Failed),
  conclusão da thread e abertura/persistência do cofre — sem segredos.
- Evidências (capturas sanitizadas — senha sempre mascarada, sem
  chaves) movidas para `~/projetos/tese/evidencias/2026-09-12-gui-ssh/`
  (fora de /tmp); verificação de sanitização no relatório da rodada.

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
