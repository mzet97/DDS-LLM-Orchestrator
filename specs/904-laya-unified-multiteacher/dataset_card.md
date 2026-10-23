# dataset_card — T904 Unified (preenchido no fechamento do M2/M3)

## Composição (partições)

| Partição | Fonte | Linhas (alvo) | Rótulos | Tier |
|---|---|---|---|---|
| C1 contrastiva nova | gerador .61 gemma-4-12b (política v3, dupla anotação 12%) | ~3400 | 12 qids + evidência | silver / silver-dual / silver-adjudicated |
| C2 contrastiva t901 | `t901/contrastive.jsonl` | 270 | 12 qids | reused-reviewed |
| C3 hard negatives | determinístico (`build_hardnegs.py`) | 70 | 12 qids (construção com sinal explícito) | template-hardneg |
| B replay | `calib_v3/dataset_calib_v2.jsonl` (sintético gemma4) | 2000 | 12 qids | reused-synthetic |
| A real | `v4_train.jsonl` SPROUT (subamostra balanceada, rótulo SÓ qos) | ≤1500 | qos_profile | reused-real |
| D distill | subconjunto silver de C1 (+C2) p/ soft targets dos 3 professores | ~1800 | gold + expected | idem C1 |

## Licença e proveniência

- Gerados nesta campanha (gemm4 na .61): uso do projeto; sem dados privados.
- SPROUT (`CARROT-LLM-Routing`): **sem campo de licença explícito no card** — caveat já
  registrado na calibração v4; usado em treino com atribuição de fonte nas tags.
- TaskBench/WorFBench (partição E): NOT_RUN nesta campanha.

## Proibições respeitadas

golden450 (`1c01ae3e…`), sealed T902 (`db457693…`), sealed T903 (`7578942a…`),
`v4_test.jsonl` — nenhum entra em treino/ajuste/seleção/teste; auditoria 8-gram contra
`v4_test` executada no builder.

## Idiomas / balanceamento

pt-BR + en mistos; distribuição de qos reportada em `datasets/unified_pool.jsonl.summary.json`
e no `split_manifest.json` por split. Contagem por qid/classe (não só linhas) no fechamento.
