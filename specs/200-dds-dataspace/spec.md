# Spec 200 — Camada DDS (dataspace)

**Fase:** 2 · **Crate:** `dds-dataspace` · **Depende de:** 000, 010 · **Substitui:**
`src/orchestrator/dds_backend/` (~3,4k LOC) — onde o GIL mais dói.

## Por quê
É o plano de dados: readers/writers, caches, monitor de QoS, liveliness. Em Python é
limitado pelo GIL (G1,G3–G7). Em Rust vira event-driven, lock-free e zero-copy.

## O quê (requisitos)
- **REQ-301 — Ciclo de vida.** `DataSpace::new(domain, role_strength)` cria participant,
  publisher, subscriber, tópicos e readers/writers com as QoS de `dds-contract`. *Aceite:*
  criação/encerramento limpos; teste sobe e derruba sem vazar entidades.
- **REQ-302 — Leitura por evento.** Readers expõem `async Stream` (WaitSet/`take_aiter`) —
  **zero polling**. *Aceite:* uma amostra publicada acorda o stream sem busy-wait; teste mede
  latência de wakeup.
- **REQ-303 — Zero-copy.** Usar loans (`take_loan`) no hot path onde a crate permitir.
  *Aceite:* caminho de leitura de TaskOutput sem alocação por amostra (documentar/medir).
- **REQ-304 — Caches concorrentes.** Caches de Task/Agent/Output em `dashmap` (sharded);
  leitura de agente **não** serializa com escrita de task. *Aceite:* teste concorrente
  (múltiplos leitores/escritores) sem corrupção nem contenção global.
- **REQ-305 — Pool de writers.** Escrita por um **pool** (MPMC) com backpressure explícito
  (maxsize + política), não uma thread única. *Aceite:* teste de streaming sustenta a taxa;
  backpressure ativa sob sobrecarga (sem crescimento ilimitado).
- **REQ-306 — Ownership por papel + Task imutável.** Strength por papel; `Task` compartilhado
  como imutável (`Arc<Task>`) → a corrida estrutural (C1) e as guardas anti-regressão do
  Python **somem**. *Aceite:* teste de disputa sem regressão de estado; sem guardas.
- **REQ-307 — Liveliness nativa.** Listener `on_liveliness_changed` (seguro em Rust, sem o
  deadlock de GIL do Python) dispara failover; `_check_agent_liveliness` só como fallback.
  *Aceite:* SIGKILL de um publicador dispara o callback (teste multi-processo se possível).
- **REQ-308 — Monitor de QoS.** Detectar deadline perdido (listener) e gaps de reliability.
  *Aceite:* teste sintetiza um gap/deadline e afirma a detecção.
- **REQ-309 — Contract tests A/B.** A mesma bateria roda contra um `InMemoryDataSpace` (mock)
  e o `DataSpace` DDS real — para o mock não divergir. *Aceite:* a bateria passa nos dois.
- **REQ-310 — Orçamento.** Propagação de estado de Task (mesmo host) **< 5 ms p99**.
  *Aceite:* bench com número real no REPORT.

## Fora de escopo
- Scheduler/registry/selector (Fase 3). Lógica de decisão de QoS (é do `qos-nfcm`).
