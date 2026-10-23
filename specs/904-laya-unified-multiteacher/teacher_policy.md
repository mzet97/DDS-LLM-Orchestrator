# teacher_policy — T904 Unified (rascunho; CONGELA no dev, antes do sealed)

## Professores (SHA verificado em checkpoint_inventory.json)

| Teacher | SHA | Competência REPORTADA | Admissão |
|---|---|---|---|
| t900 `35813054…` | verificado | planning/complexity/caps (holdout T900) | teacher |
| t902b `5d7dd652…` | verificado | urgency/QoS (sealed T902) | teacher |
| t903c `7de213c7…` | verificado | STREAM P/R (sealed T903) | teacher |
| t901 | AUSENTE (VM) | — | NÃO teacher; material de auditoria (270 contrastivos revisados) |

## Protocolo

1. **Dev comum**: cada teacher roda `system_one` sobre o dev completo (mesmo input, mesmo
   QUESTIONS_V1, mesma versão SDK); acurácia por qid contra os rótulos auditados do dev.
2. `teacher_registry.json`: SHA, métricas dev por qid, **best_teacher_by_qid** (maior
   acurácia), política de conflito — congelado antes do sealed.
3. **Destilação (gold)** nas linhas distill: para cada qid, o professor VENCEDOR daquele qid
   fornece a distribuição **somente se o argmax dele concorda com o rótulo auditado** da
   linha (rótulo prevalece; teacher errado não destila). noul usa P(true) real; choice usa
   vetor completo normalizado; score usa a distribuição por nível SE a API a expõe (sim,
   `probabilities` em system_one — verificado no SDK 0.4.10).
4. Composição no treino: linhas com `gold` por qid + `expected` nos demais qids
   (`gold[qid]` prevalece no SDK — verificado em `laya/train.py::items_from_rows`), loss
   `soft-ce`. Chamamos isso de **soft cross-entropy contra distribuições de professor
   filtradas por rótulo** — NÃO de KL com temperatura nem RLCD.
5. Temperatura de destilação/suavização: NÃO usamos (soft-ce direto sobre probabilidades do
   professor já calibradas pelo próprio treino interno). Registramos a decisão aqui antes
   do treino.
