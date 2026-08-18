# Tasks 600 — Estabilidade e segurança da v1 Rust

- [x] **T-701 · Linha de base e gate DDS compilável** (REQ-607/608)
  - Aceite: teste inicial registra o erro de tipo em `SharedWaitSet`; correção mínima
    restaura `cargo test -p agent --features dds` até o próximo red legítimo.

- [x] **T-702 · Contrato seguro de `DdsType` e construtores raw** (REQ-601/603/607)
  - Aceite: teste negativo bloqueia impl/layout inválido pela API segura; derive/idlc e
    round-trips existentes continuam verdes.

- [x] **T-703 · Ownership de `WriteLoan`** (REQ-602/607)
  - Aceite: loan mantém a entidade necessária viva mesmo após Drop do handle original;
    escrita/return e drop são exercitados sem double-return.

- [x] **T-704 · Lifecycle de filtros e callbacks FFI** (REQ-604/607)
  - Aceite: contratos de exclusividade/ownership são impostos pela API; panic permanece
    contida no boundary C e a suíte cobre clear/drop.

- [x] **T-705 · Correção do `ParticipantPool`** (REQ-605)
  - Aceite: `get` reusa o participante do domínio e outra operação do pool progride
    enquanto discovery aguarda.

- [x] **T-706 · Writer persistente e provider constraint tipado** (REQ-606/607)
  - Aceite: três literals, default `LOCAL_ONLY`, configuração explícita e múltiplas
    inferências com exatamente um writer.

- [x] **T-707 · Gates de segurança, integração e smoke** (REQ-607/608)
  - Aceite: Gates A–D executados com saídas registradas; bloqueio ambiental não é
    transformado em PASS.

- [x] **T-708 · Relatório, matriz de fidelidade e revisão independente** (REQ-608)
  - Aceite: `REPORT.md` lista achados/correções/testes/limitações; matriz da dissertação
    e cinco lanes finais de revisão têm estado terminal e evidência.
