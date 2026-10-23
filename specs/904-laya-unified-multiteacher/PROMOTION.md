# PROMOTION — T904 Unified → checkpoint vigente (2026-10-22)

**Autorizado pelo autor** após apresentação do resultado sealed (candidato domina o T900 em
todas as métricas do teste novo; gates absolutos 3/8 por serem alvos de holdout sintético).

- **VIGENTE**: arm_C `bda679e71295593904370eba5d9130609aea985f0b0d183d2527fcd398ca039a`
  + calibração pin `6e1543f3…` (do split calibration, nunca usado em treino)
- **Caminho de produção**: `~/.cache/tese-laya/checkpoints/laya-production` → `laya-t904-unified-c`
- **Baseline anterior (preservado)**: T900 `35813054…` (`teachers/t900-specialized`)
- **Validação**: shadow DDS (M8) com `shadow_only=false`; smoke no caminho de produção
  (STREAM → QI_STREAM_LIKE via `laya-production`); `tasks_seen=0/tasks_written=0`
- **Status dos gates**: os 5 gates absolutos falhados permanecem registrados como limitações
  conhecidas (urgency/planning/complexity/caps em OOD real) — promoção decorre da superioridade
  pareada sobre o baseline vigente, decisão explícita do autor; T905 pode recalibrar alvos.
- **Rollback**: apontar o worker de volta para `teachers/t900-specialized` com `--checkpoint-sha256 35813054…`
