# E2E do braço híbrido — PASS (2026-10-17)

Cadeia completa REAL em DDS (domínio 229, loopback), 112 s de ponta a ponta
para 2 workflows (1 nó PT + 2 nós EN com dependência):

intent (Python) → **decisão Laya REAL** (worker, checkpoint pinado) →
validator Rust (workflow-engine) → mapper QoS → journal SQLite (decisão
efetiva + reserva) → **planner LLM** (gemma-4-12b via LlmHttpPlanner Rust) →
DAG validator → WorkflowPlan/State publicados → materializer (laya1- task +
outbox) → **agente --adaptive** (barreira Plan/State/Task resolve o task_id
físico; claim com target exato) → **inferência real** (engine http no
llama-server) → TaskOutput + Task DONE (escritos pelo agente) → watcher
on_done → WorkflowState DONE (2/2 terminal, sem falhas).

Worker: 2 decisões OK, tasks_written=0 (fronteira mantida).

## Bugs encontrados e corrigidos nesta integração (valor de engenharia)

1. `LlmHttpPlanner::plan` (sync) chamado dentro do runtime tokio → panic
   "Cannot start a runtime from within a runtime" — resolvido com thread
   dedicada + runtime single-thread.
2. Projeção de `WorkflowState` usava `state_revision=1` fixo → conflito
   tipado no WorkflowSpace (mesma revisão, conteúdo diferente) — agora cada
   projeção publica revisão crescente por workflow.
3. Tasks publicadas com strength ORCHESTRATOR (200) impediam o claim do
   agente (Exclusive Ownership, agente=100) — engine publica com strength
   CLIENTE (10), mesmo papel do reaper legado.
4. Nodes projetados com status RUNNING(4) bloqueavam a barreira do agente
   (exige READY/PUBLISHED) — nós não-terminais publicam PUBLISHED(2).
5. **Barreira incompleta descartava a task** (corrida Task×Plan: o take()
   consome a amostra e o `continue` jogava fora) — sweeper de diferidas
   re-checa a cada 1 s até a barreira completar (semântica V06 "bloqueada
   até completar", não descartada).
6. Publisher Python com TransientLocal perdia amostra quando o DataWriter
   era coletado (GC) antes da entrega — writers retidos vivos.
