# EXP4 lado MAF — checkpoint/retomada nativa (resultados, 2026-10-06)

**Protocolo:** contraparte MAF do EXP4 definitivo DDS — mesmo ponto de falha (C, após B salvo), mesmo backend determinístico (delay 50 ms, capacity 4), mesma cadeia `seq_chain_v1`, 30 runs. Falha lógica injetada UMA vez no início de C (antes da inferência — equivale ao kill: B concluído, C sem output); retomada nativa a partir do checkpoint com a saída de B validada (`FileCheckpointStorage`, seleção do checkpoint por conteúdo causal, nunca por índice). Harness: `run_exp4_maf.py` (este diretório), venv 3.14 de `benchmarks/orchestration`.

## Resultados

| Métrica | Valor |
|---|---|
| Runs válidos | **30/30** (`resumed_output` + delta de chamadas correto) |
| **t_recovery (falha observada → retomada completa), p50** | **382 ms** |
| t_recovery min / max | 381 / 397 ms |
| t_total nominal (cadeia íntegra), p50 | 328,5 ms (322–541) |
| Não-reexecução de A/B na retomada | 30/30 (delta por rep: A:2, B:2, C:2) |
| Inferências de C | 1× por cadeia falhada (a tentativa falhada aborta ANTES da chamada) |

## Leitura

1. **Retomada custa ~16% do workflow nominal (382 vs 328,5 ms)** e recupera exatamente do ponto causal (B validado) sem re-executar A/B — a propriedade-chave do checkpoint nativo do MAF.
2. **Contraste com o DDS (EXP4 definitivo, p50 14.948,5 ms):** razão ≈ 39×. Mecanismos não comensuráveis: no MAF quem recupera é o **driver** (detém o checkpoint, re-invoca o workflow — recuperação centralizada, exige que o cliente sobreviva); no DDS a recuperação é do **data space** (reaper + republisher + claim por qualquer agente vivo, cliente só espera) — o preço é o lease de ownership do DDS (~15 s sob kill -9).
3. Faixa de variação muito estreita (381–397 ms, CV ≈ 1%) — mecanismo in-process, sem rede nem descoberta.
