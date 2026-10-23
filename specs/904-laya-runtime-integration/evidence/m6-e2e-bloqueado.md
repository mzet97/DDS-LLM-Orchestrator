# T904/M6 — E2E ativo: **BLOQUEADO (NOT_RUN)** — 2026-10-21

## Causa (única e explícita)

O gate do PROMPT T904 (§M6) exige **M5 aprovado E calibração aceita** para o caminho ativo.
O M5 foi aprovado (`evidence/m5-shadow-real.json`), porém **não existe calibração vinculada ao
T900 `35813054…`** (`evidence/calibration_manifest.md`): v1–v4 pertencem ao BASE `891102d3…`,
e reaproveitar calibração de outro checkpoint é PROIBIDO (§M4.0). Sem calibração aceita, o
validator Rust mantém toda decisão em `shadow_only=true` — a materialização de `Tasks laya1-…`
é estruturalmente impossível sem contornar o gate, o que o prompt proíbe (§8).

## O que isso NÃO significa

- NÃO houve falha técnica do pipeline: o fluxo `Workflow.Intent → T900 → Routing.Decision →
  validator Rust` funciona de ponta a ponta com identidade rastreável (M5, DDS real).
- NÃO é um bloqueio permanente: a recalibração do T900 (split independente, custo estimado no
  manifesto M4) destrava o M6 sem novas mudanças de código — o worker já publica
  `calibration_revision` quando `--calibration-path` é fornecido, e o observador aceita
  `--calibration-pin`.

## Cenários M6 (11) — estado: NOT_RUN

Todos os cenários do §M6 (workflow simples, DAG, contenção, capability ausente, modelo
incompatível, target errado, restart mantendo task_id, retry com novo task_id, cancelamento
por plan_revision, IN_DOUBT sem reexecução, resultado atrasado ignorado) **NÃO foram
executados** nesta fase. Os cenários 7–11 (journal/restart/IN_DOUBT/atraso) têm cobertura
pré-existente em testes Rust do `workflow-engine` (fase 900) — evidência herdada, NÃO
contabilizada como execução M6 da T904.

## Próxima ação para destravar

1. Autorizar e executar a recalibração do T900 (custo no manifesto M4 — CPU ~1–12 h
   conforme N, ou GPU externa).
2. Registrar o novo pin em `checkpoint_registry.md` + gates.
3. Reexecutar M5 com `--calibration-pin <novo pin>` (shadow_only=false esperado para
   decisões com o pin) e então M6 com DirectTemplatePlanner + engine mock.
