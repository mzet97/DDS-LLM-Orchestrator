# Cenários E2E avançados — 5/5 PASS (2026-10-17)

Harness: `harness/run_e2e_scenarios.py <modo>` (domínios 60-78, loopback;
llama-server local quando o cenário executa inferência). Timeline JSON +
logs por cenário nesta pasta.

| Cenário | O que prova | Gate observado |
|---|---|---|
| **retry** (B1) | FAILOVER: nó falha 1x → tentativa 2 com **NOVO task_id laya1-** | journal: [(1,failed,id-A),(2,done,id-B)] com id-A≠id-B; proxy flaky falha só a 1a req |
| **restart** (B2) | wf-run SIGKILL pós-materialize → **recover_from_journal** rehidrata (schema v2: tabela `intents`) → MESMA tentativa/mesmo task_id → DONE | journal: [(1,done,mesmo-id)]; "workflows rehidratados do journal recovered=1" |
| **k2** (B3) | 2 agentes --adaptive no mesmo domínio: claim exclusivo (Exclusive Ownership) | 2 workflows DONE; attempts: {done:2, failed:0} |
| **indoubt** (B4) | prazo curtíssimo → sweeper marca `in_doubt`; **sem retry automático** | journal: [(direct-0,in_doubt)] e SÓ 1 tentativa |
| **cancel** (B5) | `Workflow.Intent` CANCEL (command=1) → terminal CANCELLED, nada novo materializado | wf-run exit 0 com terminal=true p/ wf-cxl-1 |

## Bugs de integração encontrados e corrigidos nesta rodada

1. **Journal schema v2**: restart precisava da intent persistida — nova
   tabela `intents` gravada transacionalmente no `admit_workflow`;
   migration v1→v2 automática.
2. **recover_from_journal**: runtime rehidrata workflows do journal
   (re-planeja — direct é determinístico — reconstrói EngineState e
   **republica o Plan**: a barreira dos agentes exige Plan+State na malha
   e o crash do wf-run derrubou o TransientLocal).
3. `publish_plan` após o insert (panic "no entry found for key" quando
   chamado antes do workflow existir no map).
4. `DirectTemplatePlanner` com `max_attempts=2` (retry FAILOVER possível).
5. Agente reconstruído com feature `dds` (o binário tinha sido
   reconstruído sem dds por um build default intermediário).
6. ANSI codes do tracing quebravam o match `published=1` no harness (strip
   com regex antes de comparar).

## Gates

workflow-engine: **61/0 mock + 33/0 dds**; worker 24 OK; cenários 5/5 PASS.
