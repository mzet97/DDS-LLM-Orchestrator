# `orchestrator-studio` — desktop GUI (egui), referência técnica

> Crate: `src/rust/crates/orchestrator-studio/` · Binário `studio` · Lib
> `orchestrator_studio` · Para uso diário, ver o [Manual do Usuário](../studio-manual.md).

## Propósito

Desktop nativo do DDS Orchestrator Studio (fase 800, P1): GUI `egui`/`eframe`
para operar o laboratório DDS — descoberta de nós, inferência llama, enxame de
agentes, despacho, workflow A→B→C, catálogo compartilhado, serviços systemd
remotos, modelos GGUF e observabilidade DDS. A lib concentra a lógica de
apresentação em `state::AppState`, "pura e testada sem janela"; o binário é
"só a casca eframe".

Princípios verificados no código: **somente leitura real, sem dados fictícios**
(cada view documenta o que foi OMITIDO do mockup por não existir no fio); HTTP
bloqueante e DDS sempre em **thread de trabalho + mpsc + `poll()` por frame**
(REQ/T-820-19 — a thread de UI nunca bloqueia); segredos **só em memória**
(RNF-04/SDD §28); ações com efeito real sob **modo protegido** desarmado por padrão.

## Como rodar

```bash
cd src/rust
cargo run -p orchestrator-studio --bin studio                 # HTTP-only
cargo run -p orchestrator-studio --bin studio --features dds  # completo: Topologia/Descoberta/Ferramentas/Workflow-DDS
```

Janela 1500×950 (mín 1280×800). Tela inicial: `Topologia DDS` com `dds`,
`Visão Geral` sem `dds`; `STUDIO_SCREEN` sobrepõe. Watchdog de repaint 2 Hz em
thread própria; tema aplicado uma vez no boot. Pré-requisitos: Rust ≥ 1.85,
display gráfico; com `dds`, CycloneDDS + domínio coerente
(`STUDIO_DDS_DOMAIN` = `STUDIO_NODE_DDS_DOMAIN`).

Fluxo mínimo: 1) `studio-noded` em cada host; 2) `studio --features dds`;
3) conferir descoberta (3.10 / alvo no header); 4) auto-carga lê
Nó+Serviços+Catálogo e re-lê a cada 5 s.

## As 14 telas (Seção → view → estado)

| Tela | View `show()` | Função |
|---|---|---|
| 3.1 Visão Geral | `views/overview.rs` + `overview.rs` | Agregado somente-leitura: 6 cards + descoberta; cada card marca `stale` se a fonte nunca foi lida; `summarize()` nunca inventa |
| 3.2 Nó studio-node | `views/node.rs` + `state.rs`, `origin.rs` | RPC ao alvo: `/version` + `/operations`, token em memória, RTT + EMA, 3 cards, log com filtro + inspetor JSON, card 401×rede |
| 3.3 Inferência & Chat | `views/inference.rs` + `inference.rs` | Chat OpenAI-compatível: `ServerStatus`, probe `/v1/models`, params, transcript com stats reais, Ctrl+Enter |
| 3.4 Subir Inferência | `views/launch.rs` + `runner.rs`, `launch.rs` | Runner LOCAL `llama-server` (spawn direto): presets, CLI preview, PID/VRAM/uptime, wizard 5 etapas, console 500 linhas, SIGTERM→SIGKILL; + plano systemd-remoto legado |
| 3.5 Agentes | `views/agents.rs` + `agents.rs` | FONTE 1 DDS `AgentRegistry` × FONTE 2 HTTP `GET /api/v1/agents`; Seção A slots/lease, Seção B taxas, agregado, snapshot |
| 3.6 Despacho | `views/dispatch.rs` + `workload.rs` | `POST /api/v1/chat/completions/sync` com T1–T6 + abas payload/headers, histórico |
| 3.7 Modelos GGUF | `views/models.rs` + `models.rs` | Inventário SHA-256 + manifesto congelado, worker com progresso/cancelamento, filtros, export |
| 3.8 Serviços | `views/services.rs` + `services.rs` | Daemon control: plano pretendido×efetivo, intertravamento + confirmação 2 passos, start/stop/restart, auditoria |
| 3.9 Catálogo | `views/shared_catalog.rs` + `catalog_remote.rs` | Cliente OCC: snapshot/eventos, banner 409, gaveta com write-lock, busca, feed de mutações |
| 3.10 Máquinas | `views/machines.rs` + `machines.rs` | Descoberta `NodePresence` + registro manual (token só em memória) |
| 3.11 Topologia DDS | `views/topology.rs` + `dds_observe.rs` | Mesh desenhado, 6 contadores, filtros por tópico, 7 abas; só `read`, nunca `take` **[requer `dds`]** |
| 3.12 Workflow | `views/workflow.rs` + `workflow.rs` | Pipeline A→B→C (replica `wf-run run_seq`, prompts congelados), auditoria, painel de exceção |
| 3.13 Ferramentas | `views/tools.rs` + `DdsState` | Governança `ToolCall.Request`: cards N0/N1/N2, chips por status + REQ/s, inspetor REQUEST×RESPONSE (cap 8 KB) **[requer `dds`]** |
| 3.14 Logs da GUI | `views/logs.rs` + `studio_log.rs` | FIFO 500: stream, KPIs, filtros, pausa, export `.log`, inspetor forense |

Navegação: `NAV_GROUPS` (5 grupos), `Section`, despacho central; cada tela tem
`id_salt` de scroll próprio. App: header 44 px, sidebar 248 px (52 recolhida),
status bar 56 px.

## Estados e padrão async

Todos seguem thread + mpsc + `poll()` + `busy`: `state` (leitura do nó, RTT+EMA
α=0.3), `discovery` (worker contínuo, lease 10 s, auto-seleção 1º online,
tokens `~/.config/studio/hosts/<host>.token`), `dds_observe` (`DdsSnapshot` 7
coleções, 1 DataSpace efêmero, 7 drenos), `inference` (timeout 120 s),
`runner` (SIGTERM+5 s→SIGKILL via `libc`), `launch`, `agents`, `models`,
`services` (`fresh_operation_id`), `machines` (`MACHINE_KIND="machine"`,
prefixo `machine:`, `MachineRecord` SEM token por construção), `workload`
(`dispatch_sync`, T1–T6), `workflow` (domínio 170, `qwen3.5-0.8b`, 120 s),
`catalog_remote` (`SharedCatalogError`: Conflict/Tombstoned/CursorExpired/
Unauthorized/Unreachable), `origin` (`NodeSummary`, `OriginError`), `overview`
(`TileHealth` Ok/Warn/Stale), `protected` (`ProtectedGuard` default DESARMADO,
`NeedsConfirmation`/`Refused`), `studio_log` (500 entradas, espelho stderr,
export). Visual: `kit`, `theme` (ciano `#00E5FF`, fontes Inter + JetBrains Mono
em `assets/fonts`), `panel_header`.

## Env vars (o bin não aceita args; tudo é env + UI)

`STUDIO_SCREEN` (3.1…3.14, tela inicial); `STUDIO_DDS_DOMAIN` (default **170**,
inválido→170); `STUDIO_TARGET_URL` (força o alvo ao aparecer na descoberta);
`STUDIO_SCROLL_Y` (fixa scroll); `STUDIO_WIN_H` (≥800, default 950; largura fixa
1500); `STUDIO_ONTOP=1`; `STUDIO_GIT_HASH` (build; badge `v1.0-{hash}`, fallback
`dev`); `STUDIO_MODELS_DIR` (vence `$HOME/tese/models`); `HOME` (tokens, modelos, DB).

Features: `default=[]` → HTTP-only (sem worker de descoberta, sem 3.13 na nav,
3.11 vira aviso, 3.12 sem corredor); `dds` → completo (worker no boot,
`dds_observe`, `wf-run` real via `client/wf_assembly`, tela inicial Topologia).

## Testes

```bash
cd src/rust
cargo test -p orchestrator-studio
cargo test -p orchestrator-studio --features dds   # + Topologia/Ferramentas/Workflow-DDS, dds_live
cargo test -p studio-node
cargo test -p studio-core
```

22 arquivos em `tests/`: `kittest*.rs` (UI headless por árvore accesskit, sem
GPU), `*_wire.rs` (fio HTTP/DDS), `dds_live.rs`, `protected_kittest.rs`,
`node_origin.rs`, `token_wire.rs`. **Exceção MSRV dev-only**: produção rustc
1.85, mas os testes exigem toolchain **≥ 1.95** (`egui_kittest =0.36.2`).
Capturas manuais: `STUDIO_SCREEN` + `STUDIO_TARGET_URL` + `STUDIO_SCROLL_Y` +
`STUDIO_WIN_H` + `STUDIO_ONTOP=1`.

## Limites (fatos do código, não do mockup)

Sem `dds` ≠ completo (selo por tela no manual). Divergências honestas: sem GUID
de servidor/nó, sem build/CUDA/VRAM/slots de inferência, sem `n_ctx` (3.3); sem
systemd/cgroup no runner, bind loopback (3.4); sem p95/jitter/NIC/fila/policy
(3.5); sem stop/tok-s (3.6); sem BLAKE3/AVX/OpenSSL, sem tópico DDS de verificação
(3.7); sem PID/subestado/enabled (3.8); sem SSE, sem writer do 409 (3.9); sem
`ToolCall/Response`, política estática/permissiva M2, sem RBAC_STRICT (3.13);
`ServerStatus` é BestEffort+Volatile (3.3). LAN sem token = boot do nó recusado.
Textos G-38/65 marcados "para validação contra o SDD". Limites: payloads DDS
8 KB, console 500 linhas, logs 500 entradas; `dispatch_sync` temp −2..=2,
max 1..=8192.
