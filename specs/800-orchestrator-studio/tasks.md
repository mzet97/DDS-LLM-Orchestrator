# Fase 800 — tasks

- [x] **T-800-01 · Guarda de revisão condicional do catálogo** (REQ-800/801)
  `RevisionGuard::publish` aceita só `base == current`, avança geração
  monotonicamente e rejeita obsoleto sem mutação. Implementado + verificado;
  comitado no merge `12c974c` (PR #8).
- [x] **T-800-02 · Auditoria P0 reutilizar × não-duplicar** (REQ-802)
  `audit.md` com file:line verificado nesta execução. Comitado no merge
  `12c974c` (PR #8).
- [x] **T-800-03 · Snapshot/cursor + tombstone no studio-core** (RF-31/36; G-49/57)
  `Catalog`: revisão por objeto, tombstone persistente (recriação exige id
  novo), snapshot+cursor contíguos, expiração fora da retenção. 7 testes
  verdes no crate. Comitado no merge `12c974c` (PR #8).
- [x] **T-800-04 · Esqueleto `orchestrator-studio` (egui) read-only real** (G-01 parcial)
  `AppState` puro reflete `Snapshot` real (`studio_core::catalog`), ordena por
  id, `refresh` substitui sem acumular, estado inicial vazio honesto; bin
  `studio` (eframe 0.36, `App::ui` + `Panel`/`CentralPanel`) só lê. 3 testes
  verdes (`cargo test -p orchestrator-studio`), clippy sem lints reais
  (só `unknown lint clippy::warnings` pré-existente do workspace), fmt limpo.
  Correção pendente de commit: import `studio_core::catalog::Catalog` em
  `main.rs` (§29 do prompt SDD).
- [x] **T-800-05 · Esqueleto `studio-node` + protocolo versionado** (G-05/06 local)
  `studio-node`: `ProtocolVersion` 1.0 com `accepts`/`check` (mesmo major,
  par nunca mais novo; incompatível bloqueia com erro tipado, §31),
  `AdminEnvelope` (`operation_id` + `AdminOp`: `Bootstrap`/`SetService`) com
  fio JSON, `OperationLog` idempotente (repetição idêntica → `AlreadyApplied`,
  mesmo id com payload distinto → `OperationIdConflict`, serviço alheio →
  `OutOfScope`) + `reconcile` por id. 8 testes verdes (`--locked`), guarda
  provada por mutação, clippy/fmt limpos. Sem commit (§29).
  Pendente p/ G-05/06 integral: persistência (`studio-storage`), systemd e
  2º host/VM (bloqueado por ambiente).
- [x] **T-800-06 · Transporte HTTP localhost do `studio-node`** (P2; G-05/06)
  `server.rs` (axum 0.7): `GET /version`, `POST /apply` (gate de protocolo +
  aplicação idempotente), `GET /operations/:id`; erros do domínio viram
  status via `match` exaustivo (400 `incompatible_protocol`, 403
  `out_of_scope`, 404 `unknown_operation`, 409 `operation_id_conflict`).
  Bin `studio-noded` (porta via `STUDIO_NODE_PORT`, serviços via
  `STUDIO_NODE_SERVICES`). 14 testes verdes (`--locked`: 6 de servidor via
  `tower::oneshot` em ciclo real request/response). E2E com servidor
  destacado + curl: version/applied/already_applied/reconcile/403
  confirmados; servidor encerrado após a prova. clippy/fmt limpos
  (corrigido `clippy::double_must_use` em `router`). Sem commit (§29).
  Nota de dívida paga em T-800-07 (split abaixo).
- [x] **T-800-07 · Split `server.rs` + persistência do log** (P2; G-05/06)
  Testes do servidor movidos p/ `tests/server_api.rs` (`server.rs` 246→119
  LOC). `OperationLog` serializável com `save`/`load` JSON
  (`NodeError::Storage`); `NodeState::with_db` carrega no boot ou inicia
  novo, corrompido falha rápido; `POST /apply` persiste após aplicar
  (falha → 500 `storage_error`); `STUDIO_NODE_DB` no `studio-noded`.
  17 testes verdes `--locked` (10 lib + 7 integração, incl. restart que
  recarrega via HTTP). clippy/fmt limpos. Sem commit (§29).
- [x] **T-800-08 · Origem remota GUI→nó** (P1/P2; G-01)
  `studio-node`: `GET /operations` (lista ordenada por id).
  `orchestrator-studio`: `origin.rs` (`fetch_node_summary` bloqueante com
  timeout 3s + gate `NODE_PROTOCOL_VERSION.accepts`; `Incompatible`
  bloqueia, `Unreachable` preserva estado), `AppState::refresh_from_node`
  com status vivo, GUI com campo de URL + botão "Conectar ao nó".
  Render vazio do T-800-04 confirmado visualmente pelo usuário ao iniciar.
  5 testes da origem verdes (2 integração contra servidor efêmero real;
  `spawn_blocking` documentado p/ runtime tokio). clippy/fmt limpos.
  Instância viva em 127.0.0.1:4317 reimplantada com o endpoint novo:
  `/operations` retorna o `manual-1` persistido do processo anterior.
  Sem commit (§29).
- [x] **T-800-09 · Painel de inferência viva no GUI** (P5; G-12/13)
  `inference.rs`: `Role`/`Message`/`ChatRequest` (temperatura e máx. tokens
  atravessam o corpo do `POST /v1/chat/completions`), `list_models`,
  `InferenceState` (painel com URL, combo de modelos, sliders, prompt e
  resposta; erro vira texto, nunca vazio mudo). GUI com seção recolhível
  "Inferência". 4 testes de fio verdes (stub HTTP real em porta efêmera:
  travessia de parâmetros capturada no corpo) + smoke vivo gated por
  `STUDIO_LIVE_LLAMA=1` contra llama-server 8082 (Qwen3.5-0.8B): modelos +
  geração OK. Lio: modelo reasoning com budget curto retorna `content`
  vazio (`reasoning_content` consome tokens) — asserção corrigida p/ o que
  o contrato garante. 26 testes verdes `--locked`, clippy/fmt limpos.
- [x] **T-800-10 · Painel de agentes vivos** (P6; G-15/16)
  `agents.rs`: `AgentInfo` cru do `GET /api/v1/agents` (sem semáforo
  inventado) + `AgentsState`; GUI com tabela (slots, concluídos, falhas,
  latência). 2 testes de fio + erro tipado. Lido ao vivo: `agent-rust-sdd`
  e `agent-rust-01` (16 concluídos) no orquestrador 8085.
- [x] **T-800-11 · Despacho real via orquestrador** (P6/P8; G-25/26)
  `workload.rs`: `dispatch_sync` no `/api/v1/chat/completions/sync`
  (`Completed` com agente/latência, `Failed` com motivo, status bizarro vira
  erro tipado) + `DispatchState`; GUI com painel de despacho.
  3 testes de fio + smoke vivo `STUDIO_LIVE_DISPATCH=1`: tarefa concluída
  por `agent-rust-01` em 582ms. 15 testes do Studio verdes, gates limpos.
- [x] **T-800-12 · Sessão multi-turn + tabela de operações** (P8; G-23)
  `InferenceState` com `history`: Enviar anexa user→resposta no fio, erro
  descarta só o turno falho, "Nova sessão" limpa; GUI com transcript em
  scroll. Tabela de operações do nó (`op_summary` exaustivo) no painel do
  nó. Teste de fio: 2 envios → 2º corpo com 3 mensagens. 16 testes verdes,
  gates limpos.
- [x] **T-800-13 · Topologia DDS ao vivo no GUI** (P7/P9; G-41/42)
  `dds_observe.rs` (feature `dds`, HTTP-only sem ela): `observe()` abre um
  `DataSpace` efêmero (ownership 0, só leitura) e drena agentes/tool
  calls/métricas na mesma janela; `DdsState` + painel Topologia (domínio,
  janela, grades). Lio: `DataSpace::new` exige contexto Tokio (criar dentro
  do `block_on`) e `CYCLONEDDS_URI` explícito como nos vivos. Smoke
  `STUDIO_LIVE_DDS=1` no domínio 42: agentes reais observados. `main.rs`
  271→55 LOC (split em `views/`: node/catalog/inference/agents/dispatch/
  topology). Gates limpos com e sem `dds`.
- [x] **T-800-14 · Inventário de modelos GGUF** (P4; G-09)
  `models.rs`: `inventory()` lista `.gguf` com tamanho + SHA-256 em blocos
  (artefato ≠ instância carregada) + `ModelsState`; painel Modelos com
  diretório editável. 2 testes (fixtures: só-gguf, tamanho, digest estável
  e determinístico, erro tipado em dir ausente). Fixture corrigida por
  contagem real de bytes. Diretório real tem GGUFs legíveis. Binário DDS
  rebuiltado com o painel.
- [x] **T-800-15 · studio-noded sob systemd (user)** (P2)
  `packaging/systemd/studio-noded.service` (user unit: `Restart=on-failure`,
  singleton pelo gerenciador, `EnvironmentFile`) + env de exemplo.
  Binário instalado em `~/.local/bin`, unit habilitada e **ativa**, 1
  listener em 4317 servindo `/version`. Lio: `--now` falhou sem user
  manager e houve crash-loop contra instância setsid antiga — resolvido
  entregando a porta ao systemd. DB persistente agora em
  `~/.local/share/studio-node/operations.json` (DB de teste em /tmp
  aposentado; GUI mostra 0 ops até novos applies).
- [x] **T-800-16 · Plano pretendido × efetivo (G-07)** (P2)
  `probe.rs` (`Probe`, `SystemdProbe` real, `FakeProbe`), `wanted()` com
  índice de inserção (`order`, `#[serde(default)]` lê DBs antigos),
  `GET /services` (só lê, nunca altera o host), cliente `services.rs` +
  painel Serviços com coluna de divergência. 3 testes novos (wanted ordena
  o último, endpoint com fake, fio+erro). Vivo sob systemd: `dds-agent`
  `wanted:true/active:false` sobrevivendo ao restart (lio: binário
  instalado era pré-persistência — reinstalado). 41 testes verdes, gates
  limpos. Binário DDS rebuiltado.
- [x] **T-800-20 · Atuação idempotente start/stop** (P2/RF-05; G-05/06)
  `actuator.rs` (`Actuator`, `SystemdActuator` real, `FakeActuator`),
  `POST /services/:service/start|stop` com `operation_id`: registra,
  age só se diverge, persiste; repetido convergido não reexecuta;
  403 fora de escopo, 404 ação boba, 502 falha do gerenciador. GUI com
  botões ▶/■ por linha. Testes com fakes (incl. sem duplicar registro).
  Vivo: stop converge trivial, start em unidade inexistente → 502 honesto
  com intenção registrada (diff visível). `server.rs` 349→207 LOC (split
  `routes_services`/`routes_catalog` + reexports). Gates limpos.
- [x] **T-800-17 · Autoridade de catálogo compartilhado no nó** (P2a; G-44/45/47/49/57)
  serde nos tipos de fio do `studio-core` (doc de deps atualizado);
  `catalog_auth.rs` com journal JSONL event-sourced (append+sync, replay
  no boot, falha rápido em corrupção, `#[serde(default)]` p/ DBs);
  endpoints `POST /catalog/publish|delete`, `GET /catalog/snapshot`,
  `GET /catalog/events?since=` com 409/410/410 expirado. 3 testes de
  autoridade + ciclo de fio (cria→409 obsoleto→atualiza→snapshot→delete→
  410 tombstone→3 eventos). Vivo sob systemd: ciclo completo + replay do
  journal no restart (tombstone preservado). 32 testes node+core, gates
  limpos (só E0602 pré-existente).
- [x] **T-800-18 · GUI publica no catálogo compartilhado** (P2a; G-44/45)
  `catalog_remote.rs` (snapshot/publish/delete/events com `current`
  estruturado no 409 — `details` aditivo no `ApiErrorBody` do nó) +
  `SharedCatalog` (conflito relê snapshot, nunca silencia) + painel com
  formulário id/valor/base. Teste contra o **router real** do nó
  (multi-GUI: cria→409 com vigente→snapshot→delete→410→2 eventos).
  46 testes verdes, gates limpos, binário DDS rebuiltado.
- [x] **T-800-19 · Descoberta DDS + packaging desktop** (P3a/P10; G-43)
  `discovery_row` + 4º dreno no `observe` (dedup por `event_id`) + grade
  Descoberta na Topologia; `.desktop` instalado em
  `~/.local/share/applications` validado por `desktop-file-validate`.
  Ao vivo: 2 agentes, 0 tools/métricas/descobertas em malha estável
  (descoberta só trafega em join/leave — painel honesto vazio).
  Evidência GPU (G-14 parcial): `rocm-smi --showpidgpus` mostra o
  llama-server (PID 1001835, build ROCm) com GPU[1] e 7.4GB VRAM em uso;
  geração real de 64 tokens OK. 26 testes do Studio, gates limpos.
- [x] **T-800-21 · Inventário GGUF sem travar + navegação lateral** (P4; §30)
  Causa do travamento: SHA-256 de 70GB (15 arquivos) rodava na thread de UI.
  Agora: listagem instantânea (só metadados) + hash em thread dedicada com
  barra de progresso, `request_repaint` por frame e botão Cancelar; layout
  refeito com `Panel::left` de navegação (Catálogo/Nó/Inferência/Agentes/
  Despacho/Modelos/Serviços/Compartilhado/Topologia) + painel central com
  rolagem, conforme §30. 26 testes do Studio, gates limpos.
- [x] **T-800-22 · Visão geral (somente leitura)** (§9.2)
  Cartões Nó/Serviços/Agentes/Modelos/Inferência com fonte e `stale`
  explícito; lista vazia sem erro = "nunca lido", nunca zero inventado.
  Tela inicial padrão do Studio. 3 testes do agregado.
- [x] **T-800-23 · Subir inferência: plano → aplicar → prova** (§9.3, P2 local)
  Assistente com unidade escolhida só da lista lida do nó, prévia legível
  antes de qualquer efeito, execução em thread (start idempotente, espera
  active 60s, lista modelos, prova de geração real temp 0/32 tokens).
  Ao vivo: modelos listados e geração real OK em :8082
  (Qwen3.5-0.8B, 32 tokens). Criar unidade nova com parâmetros arbitrários
  segue inexistente no nó — declarado no plano, sem simular.
- [x] **T-800-24 · studio-noded com bind remoto + deploy na .62** (§34)
  `STUDIO_NODE_BIND` (padrão localhost; 2 testes). Deploy em 192.168.1.62
  (Ubuntu 24.04, peer do orchestrator): binário 1.7MB em ~/dds-llm-rust,
  unidade systemd `studio-noded` ativa com bind na LAN e DB persistido.
  Ao vivo: /version 1.0, /services com dds-agent wanted=null/active=false,
  /operations vazio. Sem atuação remota nesta tarefa (só leitura, G-46).
- [x] **T-800-25 · studio-noded também na .61 (GPU)** (§34)
  Mesmo binário/unidade do T-800-24 com bind na LAN da .61 (RTX 3080).
  Ao vivo: /version 1.0 nas duas (.61 e .62), dds-agent parado sem
  pretendido em ambas. Dois nós administráveis + local = 3 origens.
- [x] **T-800-26 · Registro de nós conhecidos na GUI** (P3-admin parcial)
  `nodes.rs`: adicionar/selecionar/remover `(apelido, URL)` digitados,
  sem varredura e sem SSH; painel Nós virou registro + conexão. 4 testes.
  G-02/G-04 seguem bloqueados (cadastro SSH com host key fora do escopo).
