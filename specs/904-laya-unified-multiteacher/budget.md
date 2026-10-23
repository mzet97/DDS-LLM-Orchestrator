# budget — T904 Unified (autorizado pelo autor em 2026-10-21)

- **Teto Colab: 20 CU** (decisão do usuário; saldo inicial 368 CU, CLI 0.7.4, taxa T4 ≈ 1.07/h quando ativa).
- Estimativa: upload 3 professores (842 MB cada, 41 partes) + inferência de professores + 4 treinos (~1 h/braço) + avaliações/dev/sealed ⇒ ~6–12 CU.
- `colab usage` conferido a cada fase; **ao atingir 20 CU: parar, salvar estado, relatar**. Autorização NÃO cobre: apagar VMs, migrar produção, treinar além do teto sem novo consentimento.
- Geração M1: **VM .61** (gemma-4-12b :8083, decidido pelo autor; sem custo monetário). Enquanto gera, NENHUM outro cliente na .61 (GPU -np 1; 2 clientes = hang — lição B-BENCH).
- E2E M9: .61 (após geração concluída, sem conflito de GPU).
- Disco: SMB com ~22 G livres — artefatos pesados intermediários em `~/.cache` (410 G livres); entrega final (~1,7 GB: checkpoint + ZIP) cabe no SMB com monitoramento.

## Uso registrado (atualizar por fase)

| Fase | CU antes | CU depois | Δ |
|---|---|---|---|
| (linha base 2026-10-21 15:5x) | 368.00 | — | — |
