# Spec 100 — Agente (proxy de execução)

**Fase:** 1 · **Crate:** `agent` (bin) · **Depende de:** 000, 010, e um mínimo de 200/300
(cliente/dataspace básicos) · **Substitui:** `src/orchestrator/agent/` (~2,0k LOC).

## Por quê
O agente é o componente mais **quente** (streaming, per-amostra) e mais **isolado** (fala
DDS com o resto; a inferência é do llama-server C++). Migrar 1 agente Rust ao lado de N
agentes Python é o experimento de maior ROI e menor risco.

## Comportamento atual (paridade — ver Python)
`agent/task_consumer.py`: assume task PENDING compatível (`assign`+`write_task`+
`confirm_task_claim`), executa, publica TaskOutput. `agent/dds_llm_engine.py`: ponte ao
llama-server via `LLM.*`, event-driven (`Condition`). `agent/main.py`: heartbeat dedicado,
`--slots 1` (serial), `--specialization`.

## O quê (requisitos)
- **REQ-201 — Claim de task.** Assinar `Tasks` (WaitSet/async stream), achar uma PENDING
  compatível com a especialização, escrever ASSIGNED com `assigned_agent`. *Aceite:* teste
  com stub: task PENDING compatível é assumida; incompatível é ignorada.
- **REQ-202 — Confirmação de ownership.** Após ASSIGNED, confirmar via readback que este
  agente detém a instância antes de executar (evita execução dupla). *Aceite:* teste de
  disputa: 2 agentes na mesma task → só 1 executa.
- **REQ-203 — Ponte ao llama-server (C++).** Publicar `LLMInferenceRequest` e receber
  `LLMInferenceResult`/stream via DDS. *Aceite:* com o llama-server DDS, uma inferência real
  completa; sem ele, um mock de engine responde (teste isolado).
- **REQ-204 — Streaming de saída.** Publicar `TaskOutput` em chunks com `seq_num` crescente,
  por um **pool de writers** (sem thread única). *Aceite:* N chunks ordenados sem gaps.
- **REQ-205 — Heartbeat + liveliness.** Thread/`tokio task` dedicada publica `AgentState` a
  cada 5s; QoS `Liveliness.ManualByTopic(10s)`. *Aceite:* heartbeat não congela durante
  inferência longa; teste afirma publicação periódica.
- **REQ-206 — Timeout por deadline.** O tempo-limite da inferência deriva do deadline da
  task (margem). *Aceite:* engine desiste antes do reaper reatribuir (teste com deadline curto).
- **REQ-207 — Especialização/roteamento.** Respeitar `target_agent` e especialização
  (TEXT/VISION). *Aceite:* teste de roteamento (paridade com o fix do dispatcher Python).
- **REQ-208 — Coexistência (A/B).** 1 agente Rust + N Python disputando 100 tasks → **zero
  execução dupla**, zero regressão de estado. *Aceite:* teste multi-processo (ou multi-runtime).
- **REQ-209 — Orçamento de desempenho.** Propagação claim→execução e CPU **medidos** e
  reportados vs o agente Python. *Aceite:* REPORT com números; propagação < 5 ms p99 (meta).

## Fora de escopo
- Reescrever o llama-server (C++). Scheduler/registry (é do orchestrator, Fase 3).
