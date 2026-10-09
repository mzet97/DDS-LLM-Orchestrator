# `studio-node` — nó administrativo HTTP + presença DDS

> Crate: `src/rust/crates/studio-node/` · Binário `studio-noded` · Lib `studio_node` ·
> Fase 800 (P2/P2a), presença T-890, auth T-840-01, SQLite T-890-05.

## Propósito

Nó local do Studio. Expõe protocolo administrativo versionado em HTTP localhost
(axum), log idempotente de operações com reconciliação (G-05/06, G-50), atuação
start/stop idempotente em serviços próprios via systemd user (RF-05), catálogo
compartilhado autoritativo com journal JSONL event-sourced (P2a), persistência
do log (SQLite default, JSON legado) e heartbeat de presença DDS no 19º tópico
canônico `Studio.NodePresence` (T-890; mDNS eliminado — DDS é requisito mínimo
por máquina).

## Como rodar

```bash
cd src/rust
cargo run -p studio-node --bin studio-noded                                      # 127.0.0.1:4317, HTTP-only
cargo run -p studio-node --features dds --bin studio-noded                      # + presença DDS (com STUDIO_NODE_DDS_DOMAIN)
STUDIO_NODE_BIND=0.0.0.0 STUDIO_NODE_TOKEN=<≥16> STUDIO_NODE_SERVICES=dds-agent,llm-local \
  STUDIO_NODE_DDS_DOMAIN=0 STUDIO_NODE_PUBLIC_URL=http://192.168.1.62:4317 \
  STUDIO_NODE_ID=no-lab-1 cargo run -p studio-node --features dds               # LAN completo
```

Sem env/feature DDS, segue HTTP-only; binário sem feature `dds` mas com env
avisa e desativa presença. Falha de DDS nunca derruba o nó. Fora de localhost
sem token ≥ 16 chars, o boot falha.

| Env | Default | Efeito |
|---|---|---|
| `STUDIO_NODE_PORT` | `4317` | Porta HTTP (parse inválido aborta) |
| `STUDIO_NODE_BIND` | `127.0.0.1` | IP de bind |
| `STUDIO_NODE_TOKEN` | ausente (só localhost) | Bearer [REDACTED] tudo exceto `/version` (SHA-256 + tempo constante) |
| `STUDIO_NODE_SERVICES` | `dds-agent` | Serviços próprios (CSV); fora do escopo → 403 |
| `STUDIO_NODE_DB` | `$HOME/.local/share/studio-node/operations.db` | `.json` → backend JSON, demais → SQLite; sem `HOME` → volátil com aviso |
| `STUDIO_NODE_DDS_DOMAIN` | ausente (HTTP-only) | Domínio DDS; lixo desativa sem derrubar |
| `STUDIO_NODE_PUBLIC_URL` | `http://{bind}:{port}` | URL anunciada (NAT/overlay) |
| `STUDIO_NODE_ID` | derivado URL/hostname | `node_id` (@key; único por instalação) |

Nenhuma flag CLI — tudo por env. Persistência: SQLite (WAL, tabelas
`log_meta`/`operations`, escrita atômica em transação, migração não-destrutiva
JSON→SQLite com rename para `.imported`, só se banco virgem); falha de disco →
volátil com aviso, nunca pânico.

## API HTTP (tudo exceto `/version` exige Bearer [REDACTED] configurado → 401 `unauthorized`)

| Rota | Método | Corpo/Query | Resposta |
|---|---|---|---|
| `/version` | GET | — | `{"major":1,"minor":0}` (sempre 200, aberto) |
| `/apply` | POST | `AdminEnvelope{protocol, operation_id, op}` | `{"outcome":"applied"\|"already_applied","record"}`; 400 `incompatible_protocol`, 409 `operation_id_conflict`, 403 `out_of_scope`, 500 `storage_error` |
| `/operations` | GET | — | `[{id,op}]` ordenado por id |
| `/operations/:id` | GET | — | `{id,op}` ou 404 `unknown_operation` |
| `/services` | GET | — | `[{service,wanted,active}]` ordenado (só lê) |
| `/services/:service/:action` | POST | `{"operation_id"}`; action=start\|stop | `{service,wanted,active,acted}` (`acted:false` se já convergido); 404 `unknown_action`, 403 `out_of_scope`, 502 `actuator_failed` |
| `/catalog/snapshot` | GET | — | `{items:[{id,value,revision}],cursor}` |
| `/catalog/events` | GET | `?since=N` (default 0) | `[{seq,kind,id,revision}]` ou 410 `cursor_expired` |
| `/catalog/publish` | POST | `{id,base:null\|N,value,generation}` | `{"revision":N}`; 409 `revision_conflict`/`stale_generation`, 410 `tombstoned` |
| `/catalog/delete` | POST | `{id,base:N}` | `{"revision":N}`; 409/410 idem |

Protocolo: `NODE_PROTOCOL_VERSION = 1.0`; compat = mesmo major + peer.minor ≤ nó.

## Componentes

- **`OperationLog`**: `apply` idempotente por id (repetição idêntica →
  `AlreadyApplied`; payload distinto → `OperationIdConflict`; serviço alheio →
  `OutOfScope`), `reconcile`, `wanted` (último `SetService`), retenção FIFO
  1000, `restore`/`save`/`load`.
- **`CatalogAuthority`**: `studio_core::Catalog` + journal JSONL write-ahead com
  `sync_all` por mutação; falha de append → rollback por replay; boot reconstrói
  por replay, corrompido falha rápido. Journal em `<stem>.catalog.jsonl`.
- **Bordas SO**: `Probe::is_active` (`systemctl --user is-active --quiet`),
  `Actuator::set_active` (`systemctl --user start|stop`); `FakeProbe`/`FakeActuator`
  p/ testes; ambos bloqueantes → sempre via `spawn_blocking`, nunca sob o Mutex.
- **Presença DDS** (`presence.rs`): heartbeat 5 s (lease ManualByTopic 10 s,
  KL1) via `DataSpace` dedicado, ownership 0 (não disputa `Tasks`); publica
  `StudioNodePresence{node_id(@key),url,protocol_*,token_required,services_hint,
  last_seen_ns}`; erro de escrita loga e continua.
- Lib: `server::{router,NodeState,...}`, `protocol::{...}`, `operations::{...}`,
  `catalog_auth::{...}`, `storage::{Storage,JsonStorage,SqliteStorage}`,
  `presence::{...}`, `probe::{...}`, `actuator::{...}`.

## Testes

```bash
cd src/rust
cargo test -p studio-core -p studio-node
cargo test -p studio-node --features dds          # + presença (presence_sample, node_id)
CYCLONEDDS_STATIC=1 cargo test -p studio-node --features dds   # DDS real em SMB/CIFS
```

Integração: `server_api` (ciclo HTTP real: version/apply idempotente/409/403/400/
reconciliação/restart/services/catálogo/atuação) e `auth` (401/200). Units:
protocolo, operações, journal, storage, presença, fakes.

## Limites

- **Sem TLS**: Bearer [REDACTED] HTTP puro; em LAN o token trafega em claro.
- Retenção divergente sem doc cruzada: ops FIFO 1000 vs catálogo 64 eventos;
  ids antigos viram `unknown_operation` (aceitável por desenho).
- Journal do catálogo sem teste de integração HTTP (só unit).
- `lib.rs` doc-comment desatualizado (ainda diz default JSON; vigente é SQLite).
- Caminho `acted:false` real sem cobertura de integração (FakeProbe estático);
  protocolo de lock curto sem teste de contenção.
