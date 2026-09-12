# Fase 800 — tasks

- [x] **T-800-01 · Guarda de revisão condicional do catálogo** (REQ-800/801)
  `RevisionGuard::publish` aceita só `base == current`, avança geração
  monotonicamente e rejeita obsoleto sem mutação. Implementado + verificado;
  commit pendente de autorização (§29 do prompt SDD).
- [x] **T-800-02 · Auditoria P0 reutilizar × não-duplicar** (REQ-802)
  `audit.md` com file:line verificado nesta execução. Sem commit
  (pendente de autorização, §29 do prompt SDD).
- [x] **T-800-03 · Snapshot/cursor + tombstone no studio-core** (RF-31/36; G-49/57)
  `Catalog`: revisão por objeto, tombstone persistente (recriação exige id
  novo), snapshot+cursor contíguos, expiração fora da retenção. 4 testes
  verdes. Sem commit (pendente de autorização, §29 do prompt SDD).
- [ ] **T-800-04 · Esqueleto `orchestrator-studio` (egui) read-only real** (G-01)
- [ ] **T-800-05 · Esqueleto `studio-node` + protocolo versionado** (G-05/06)
