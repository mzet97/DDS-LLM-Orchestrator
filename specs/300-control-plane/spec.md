# Spec 300 — Control plane (orchestrator + client + gateway)

**Fase:** 3 · **Crates:** `orchestrator` (bin), `client`, `llm-gateway` · **Depende de:**
100, 200 · **Substitui:** `orchestrator/`, `client/`, `llm_gateway/`.

## Por quê
Fecha o caminho Rust: ingestão HTTP, escalonamento, registro, seleção, o **loop de controle
com o NFCM** (já pronto em `qos-nfcm`), o cliente (que resolve o deadlock de 20) e o gateway
multi-worker real.

## O quê (requisitos)

### orchestrator
- **REQ-401 — Ingestão HTTP.** `axum`/`hyper` expõe a API de submissão (paridade com o
  endpoint aiohttp). *Aceite:* `POST /api/v1/chat/completions` aceita e enfileira.
- **REQ-402 — Scheduler.** Fila de prioridade de tasks. *Aceite:* ordem por prioridade+idade.
- **REQ-403 — Registry.** Monitor de heartbeat/liveliness dos agentes. *Aceite:* agente morto
  é marcado; suas tasks reatribuídas.
- **REQ-404 — Selector/Dispatcher.** Roteamento por especialização/capacidade (paridade com o
  fix do dispatcher Python). *Aceite:* teste de roteamento TEXT/VISION.
- **REQ-405 — Loop de controle + QoS.** A cada ciclo: coletar métricas → `qos-nfcm` infere →
  salvaguarda de estabilidade → aplicar **online knobs** (não estruturais) → logar trace
  `qos_decision`. *Aceite:* teste com métricas degradadas → Failover; trace emitido.
- **REQ-406 — State machine.** Transições de Task válidas (reassign passa por `can_transition`).
  *Aceite:* transição inválida em task terminal é rejeitada.

### client
- **REQ-410 — Submissão/streaming.** `submit(task) -> Future<Result>` + stream de chunks.
  *Aceite:* submete e recebe resultado/stream.
- **REQ-411 — Sem deadlock de concorrência.** UM participante servindo N tasks async;
  **≥ 50 clientes concorrentes** sem deadlock (Python travava em 20). *Aceite:* teste de 50+.

### llm-gateway
- **REQ-420 — Multi-worker real.** N workers (`Semaphore`), sem GIL corrompendo métricas.
  *Aceite:* teste com N>1 workers processa em paralelo; métricas corretas.
- **REQ-421 — Roteamento de provedor.** local (llama-server C++) vs cloud, por
  constraint/SecurityLevel. *Aceite:* teste de roteamento.
- **REQ-422 — Cache + rate-limit + 429.** hit devolve resultado; drop por rate-limit publica
  `LLMInferenceError(429, retriable)`. *Aceite:* teste dos três caminhos.

### integração
- **REQ-430 — E2E Rust-only.** cliente→orquestrador→agente→(mock ou llama)→resultado, tudo
  Rust. *Aceite:* teste E2E verde; paridade com o E2E Python.

## Fora de escopo
Baselines Zadeh/FCM (Fase 4). Reescrever o llama-server.
