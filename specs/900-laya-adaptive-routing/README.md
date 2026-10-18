# 900 — Laya adaptive routing: dossiê SDD

Entrega documental M0, baseada no HEAD `5a7c15b0be3ad0638d8a041338eda41c9587ae03`, branch `studio/phase-800-node`. **Atualização 2026-10-17 (M1 COMPLETO):** o usuário autorizou \"fazer todos os specs\"; a sessão executora concluiu M1-01..M1-04 (contrato isolado com 3 emendas documentadas, integração dds-contract + bindings Python no worker futuro, WorkflowSpace DDS/in-memory com erros tipados, interop real Rust↔Python 4/4 e regressão dos 19 tópicos verde). M2–M7 em andamento na mesma sessão; nenhum commit criado.

**Entrada para a próxima IA:** [PROMPT_CONTINUACAO_LAYA_DDS.md](PROMPT_CONTINUACAO_LAYA_DDS.md).

| Documento | Finalidade |
|---|---|
| [REPORT](REPORT.md) | Entrega M0, verificações realmente executadas e limitações. |
| [audit](audit.md) | Diagnóstico evidence-first e inventário NFCM. |
| [spec](spec.md) | 16 requisitos e invariantes de aceitação. |
| [architecture](architecture.md) | Fronteiras, dados, claim, decisões e recuperação. |
| [contracts](contracts.md) | Proposta M0-1 dos cinco tópicos, tipos, bounds, chaves e QoS. |
| [plan](plan.md) | Sequência M0–M7, gates e rollback. |
| [tasks](tasks.md) | Ledger M0 e tarefas atômicas M1–M7. |
| [test-plan](test-plan.md) | Baseline e cenários futuros V01–V12. |
| [traceability](traceability.md) | Requisitos → artefatos → tarefas/testes → evidências. |
| [risks](risks.md) | Pendências, decisões de revisão, responsáveis e marcos afetados. |
| [evidence](evidence/README.md) | Logs, metadados e verificação de preservação. |

ADRs: [001 — Python/DDS](adrs/001-python-laya-dds-only.md), [002 — QoS](adrs/002-application-qos-versus-native-dds-qos.md), [003 — capabilities/claim](adrs/003-capabilities-and-claim-eligibility.md), [004 — DAG/recovery](adrs/004-workflow-dag-and-recovery.md), [005 — NFCM](adrs/005-nfcm-deprecation-boundary.md).

## Estado por marco

| Marco | Estado |
|---|---|
| M0 | CONCLUÍDO como entrega documental; propostas aguardam revisão humana. Ver REPORT para baseline e limitações. |
| M1 | **CONCLUÍDO (2026-10-17, 4/4 tareções)**: IDL+fixtures, build.rs/exports/mock, WorkflowSpace DDS/in-memory, interop real Rust↔Python 4/4 (V02/V03) + regressão 19 tópicos. 3 emendas documentadas. Ver REPORT §M1 e evidence/m1-01..04. |
| M2 | **CONCLUÍDO com adapter mock (2026-10-17)**: worker+CLI, perguntas V1, validator workflow-engine, dataset seed+baseline, shadow V04 PASS (tasks=0). Inferência REAL segue BLOCKED (R-LAYA). Ver evidence/m2. |
| M3 | **CONCLUÍDO (2026-10-17)**: catálogo rev 1 + elegibilidade com revogação; barreira `--adaptive` (target exato, tentativa ativa, V05/V06 in-memory); contenção K=2/K=4 + restart. DDS-real p/ M5. Ver evidence/m3. |
| M4 | **CONCLUÍDO (2026-10-17)**: mapper puro ADR-002 + telemetria V07 (33/0). Ver evidence/m4. |
| M5 | **CONCLUÍDO (2026-10-17, 60/0)**: journal SQLite/outbox/reservas, DAG validator (+orçamento agregado), engine readiness/recovery. Ver evidence/m5. |
| M6 | 01/02 congelados; **03 PILOTO REAL executado** (R-LAYA desbloqueado 2026-10-17: laya 0.4.10 + checkpoint pinado; avaliação real + 5×2 runs com manifests + shadow real PASS). Campanha N≥30 COMPLETA + **E2E do braço híbrido PASS** (wf-run) + calibração wiring (pin a235c9b9…; números indicativos). |
| M7 | **CONCLUÍDO (2026-10-17)**: 01 inventário; 02 default static + loop NFCM condicional (regressões verdes); 03 preservação verificada com hashes. |

Decisões já fechadas pelo usuário: Rust no núcleo/agentes, Laya Python, integração DDS interna, Laya como origem semântica de QoS, NFCM legado para evolução e preservação acadêmica. 2026-10-17: usuário desbloqueou pin/instalação do Laya nesta máquina e a mudança de default legado (M6-03/M7-02). Propostas para revisão: contrato M0-1, domínio adaptativo isolado, QoS de aplicação V1 e journal SQLite local com engine único. Não confundir aprovação de auditoria com aceitação dessas mudanças de compatibilidade.
