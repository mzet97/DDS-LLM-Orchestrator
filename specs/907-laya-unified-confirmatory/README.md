# T907 — Unified Laya: Confirmatory Evaluation

**Estado:** Fase A (zero-custo) em execução — 2026-11-02
**Objetivo:** Avaliação reproduzível e independente; decidir com dados se treino adicional é necessário; se sim, executar com orçamento controlado.

## Fases

### Fase A — Zero-Custo (M0-M5)
| Marco | Tarefa | Gate |
|---|---|---|
| M0 | Inventário + correção b0_reference | ✅ CONCLUÍDO |
| M1 | Avaliador único CPU/GPU determinístico | 3 execuções idênticas |
| M2 | Auditoria datasets + coverage 8/8 | Zero UNSCORABLE |
| M4 | Sealed T907 novo (famílias inéditas) | SHA congelado |
| M5 | Baselines harmonizados no mesmo dev | Tabelas pareadas |

### Ponto de Decisão
- C2 atende gates → pular para M8 (calibração) sem treino
- C2 não atende → M6/M7 (treino, ~15 CU)

### Fase B — Treino Condicional (M6-M7)
Braços A/B/C (ver PLAN.md original para detalhes)

### Fase C — Validação (M8-M11)
M8: calibração → M9: sealed único → M10: shadow DDS + E2E → M11: DECISION_T907.json

## Artefatos
- `manifests/baseline_inventory.json` — hashes verificados
- `manifests/gates_preregistration.json` — gates congelados ANTES do treino
- Ver `laya_t900_handoff/t907/` para datasets, evals e models
