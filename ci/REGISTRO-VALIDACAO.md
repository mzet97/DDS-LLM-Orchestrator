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
