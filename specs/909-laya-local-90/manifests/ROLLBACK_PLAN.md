# Rollback T909
- Produção vigente INALTERADA durante toda a campanha: T904 (bda679e7, laya-production).
- Promoção (só com aprovação explícita do autor): versão nova = laya-t909-unified com MANIFEST;
  rollback documentado = restaura symlink laya-production -> laya-t904-unified-c + calib 6e1543f3.
- Nenhum checkpoint histórico é sobrescrito; braços T909 em t909/models/arm_*.
- Se treino abortar (ROCm/térmica): BLOCKED_BEFORE_TRAINING ou retomada por época (WEIGHTS_ONLY_RESUME).
