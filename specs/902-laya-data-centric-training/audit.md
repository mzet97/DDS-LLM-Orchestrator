# Auditoria M0 (2026-10-20, somente leitura antes de GPU)

## Integridade verificada
- Checkpoint T900 local: SHA 35813054... confere (358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f), 842609220 bytes.
- Auxiliar T900: SHA 030f34b5... preservado. Base pinada: 891102d3... (adapter.py).
- HEAD rust: fc56e60, branch studio/phase-800-node. Sem sessao Colab ativa; saldo 383.92 units; CLI 0.7.4; laya==0.4.10.
- QUESTIONS_V1 rev.1 confirmado em questions.py (choice/noul/score, expected: choice=str QI_*, noul=bool, score=int ordinal 0..L-1).

## Causas falsificaveis (de T901, reproduzidas)
1. Rotulo antes do texto no gerador sintetico -> exemplos STREAM sem streaming e urgency low com "urgently". Hipotese: contrastivos com sinal observavel melhoram STREAM sem derrubar urgencia. Teste: M4/M6 no dev agrupado.
2. Vazamento por familia (290/341) -> metricas otimistas. Hipotese: split agrupado reduz metricas do seed. Teste: comparar seed no dev agrupado T902 vs golden antigo.
3. RLCD com spikes e calibracao escassa (26-41 itens) -> SoftCE pode estabilizar. Teste: ablacao C.

## Orcamento (ancoras T900/T901 medidas)
- Treino ~1300 updates T4 ~85 min; avaliacao 450 ~2-5 min/modelo. Piloto+3 bracos+finalistas: 6-9h T4 (~7-10 units). Gate: sem baseline integro ou piloto inconsistente, nao treinar.
