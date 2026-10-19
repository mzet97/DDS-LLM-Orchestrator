# Fase 910 — REPORT: Control Panel Rust

**Data:** 2026-10-18. **Executor:** agente principal (workflow multi-agente indisponível; execução inline com a mesma evidência).

## Entregue

- Crate nova `control-panel` em `src/rust/crates/control-panel` (21º membro do workspace), lib + bin `control-panel`. Nenhum arquivo de `workflow-engine`, `agent` ou `dds-dataspace` foi editado (mtimes conferidos; única edição compartilhada: registro do membro em `src/rust/Cargo.toml`).
- Módulos: `config` (env `CP_*`, listas de segredos), `catalog` + `presets` (serviços = bins Rust atuais + llama-server; presets `minimal/local-dds/full-rust`), `process` (start/stop/restart/tail, Drop mata o filho), `supervisor` (registro thread-safe, agentes dinâmicos, preset ordenado core→support→llm_backend→agent), `server` (axum espelhando `supervisor.py:216-301` + `/api/dashboard`, `/api/hardware`, `/api/chat/send`, token Bearer opcional), `hardware` (/proc + nvidia-smi/rocm-smi), `inventory` (parser Ansible INI + redação), `ansible` (allowlist, comando em lista, anti-escape, 1 run por vez), `test_runner` (descoberta/execução/histórico JSONL), `chat` (mensagens puras + `ClientChatTransport` via crate `client`, feature `dds`).

## Gates medidos

| Gate | Resultado |
|---|---|
| `cargo test -p control-panel` | **30 passed, 0 failed** (12 ansible/módulos + 7 catálogo + 5 inventário + 6 processo/API) |
| `cargo clippy -p control-panel --all-targets --all-features -- -D warnings` (com `CYCLONEDDS_STATIC=1`) | limpo |
| `cargo fmt -p control-panel -- --check` | limpo |
| Baseline Python (antes do cutover) | 42 passed, `test_chat_client.py` sem coletar (import quebrado pré-existente) |
| E2E binário (processo fake) | start/status/stop-all sem órfãos |
| **E2E DDS real** (domínio 42) | painel subiu `orchestrator` + `agent --engine mock` reais; `POST /api/chat/send` respondeu `chunk-0000…chunk-0004`; `stop-all` limpo |

## Paridade

Cada teste Python verde tem homólogo Rust nomeado em `crates/control-panel/tests/` (catalog, inventory, process_and_api, ansible_and_modules). Os 5 testes de chat do Python, quebrados na origem, foram portados corrigidos contra funções puras. Descartados por decisão de spec: templates Flask (UI = orchestrator-studio), `dependency_manager` (pip), presets que subiam o gateway Python.

## Cutover

- `src/control_panel` arquivado em `archive/control_panel-python-legado-2026-10-18/` após o E2E DDS verde.
- `docs/knowledge-base/02-src.md` e `notes.md` atualizados.
- Restam apontando para o Python arquivado (histórico, não corrigidos de propósito): `scripts/run_control_panel.sh`, `scripts/run_supervisor.sh`.

## Limites e follow-ups

- Chat DDS exercitado com agente `mock`; com engine `dds`/`http` depende do llama-server do ambiente.
- `host_status` via Ansible facts foi reduzido ao runner + inventário; coleta de facts por host (cache TTL do Python) é follow-up.
- API não expõe SSE de logs/Ansible (o Python tinha listeners); tail por polling cobre o caso atual.

## Correção 2026-10-18 (paridade com o original)

A primeira entrega foi rejeitada pelo autor por dois desvios, ambos corrigidos:

1. **Autenticação inventada removida.** O original (`archive/control_panel-python-legado-2026-10-18`) não tem nenhuma autenticação (busca por `Authorization|Bearer|login|auth` vazia). Removidos middleware, `CP_TOKEN`, campo de token da UI e a linha `CP_TOKEN` do env do serviço no .62. Nenhuma rota retorna 401.
2. **Páginas restauradas.** As 6 páginas do original voltaram como rotas próprias (`/`, `/services`, `/tests`, `/chat`, `/vms`, `/settings`) com layout/tema/toasts equivalentes, e os comportamentos: dashboard (supervisor online, presets rápidos, últimos testes, métricas), serviços (logs com auto-refresh, exportar `.log`, presets, agentes), chat SSE (send+stream, `model_required/max_tokens`, upload imagem/áudio, eventos token/final), Cluster (status de hosts com cache/refresh, `--limit`, runs com stream SSE e cancelamento), testes (categorias, run-category, detalhe, histórico) e settings (GET/POST allowlist persistido — provado após restart do serviço). CLI com `supervisor|panel|status|stop-all|catalog` via HTTP puro, sem `curl`.

Gates após a correção: 33 testes verdes, clippy all-features limpo, fmt limpo. Deploy: release no .62, `control-panel.service` ativo, todas as rotas acima respondendo 200 sem token (verificado por HTTP; navegador real indisponível nesta máquina — `chromium` ausente —, comportamento em browser declarado não verificado).

Limites que permanecem: status de host é sonda TCP/22 (não facts Ansible `setup`); linhas de run Ansible são anexadas ao fim de cada stream (stdout/stderr), não linha-a-linha; `max_tokens` é aceito na API e aplicado pelo agente, não cortado no painel.
