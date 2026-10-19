# Histórico — documentos de planejamento e auditoria concluídos

Documentos que dirigiram fases JÁ CONCLUÍDAS da migração/otimização
(2026-05 → 2026-09). Movidos para cá na limpeza de 2026-10-19 para despoluir
a raiz do workspace; o histórico git preserva a autoria original (`git log
--follow`). Continuam sendo citados por comentários de código como evidência
de decisões de design (os caminhos foram atualizados para `docs/history/…`).

| Arquivo | O que foi | Estado |
|---|---|---|
| `MIGRATION_PLAN.md` | Plano macro da migração Python→Rust (2026-05) | Concluído — fases 000–900 executadas |
| `MIGRATION_GAP_ANALYSIS.md` | Análise estática de gaps da migração | Concluída |
| `PLANO_EXECUCAO.md` | Plano de execução detalhado por WF (2026-07-17) | Concluído — WF-0..8 verdes |
| `ACTION_PLAN_DDS_IMPLEMENTATION.md` | Plano de ação da implementação DDS real | Concluído |
| `OPTIMIZATION_AUDIT.md` | Auditoria de performance (2026-07-20) | Concluída |
| `OPTIMIZATION_PLAN.md` | Plano de otimização fases 0–6 | Concluído — fases implementadas e validadas |
| `OPTIMIZATION_REPORT.md` | Relatório final da campanha de otimização (com a comparação E2E Rust×Python) | Registro histórico — citado por comentários de código |
| `debug-journal-2026-08.md` | Diário da estabilização Rust/CycloneDDS v1 (2026-08-17) | Sessão encerrada |
| `ci/PROMOCAO-tese-rust-54a7c34*.md` | Registros da promoção canário F2→F3 (2026-09-14) | Promoção concluída — o registro vivo de CI é `ci/REGISTRO-VALIDACAO.md` |

O estado ATUAL do projeto vive em: `README.md`, `CONTEXTO.md`, `AGENTS.md`,
`specs/<fase>/REPORT.md` (dossiês SDD), `docs/` (documentação oficial dos 20
crates) e — para a frente de otimizações de 2026-10 —
`specs/905-otimizacao/` + `PLANO_OTIMIZACAO_DDS.md` na raiz do tese.

## Nota sobre a duplicata `docs/rust` da raiz do tese

Até 2026-10-19 existia uma CÓPIA não versionada desta documentação em
`<tese>/docs/rust/` (criada junto com a sessão de docs de 2026-10-09; o
original versionado é ESTE `src/rust/docs/`). A cópia foi arquivada em
`<tese>/archive/docs-rust-duplicata-sem-versionar-2026-10-19/` — o canônico
passa a ser unicamente `src/rust/docs/` (revisado em PR, atualizado com as
páginas do `workflow-engine` e o comportamento O-2 do agente).
