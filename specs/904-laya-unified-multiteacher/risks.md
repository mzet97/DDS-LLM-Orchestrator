# risks — T904 Unified

| Risco | Prob. | Impacto | Mitigação | Estado |
|---|---|---|---|---|
| Sessão Colab expira no meio do treino | alta | braço perdido | checkpoint_latest por época + logs; refazer só o braço; dados/pesos na VM podem sumir ⇒ re-upload | previsto |
| Download IncompleteRead (842 MB) | média | entrega bloqueada | download em partes 21 MB + SHA VM↔local | implementado (drive_colab) |
| Gerador .61 lento/oscilar | média | dataset curto | alvo recalibrado 600 lotes×6; retomável; t901+hardneg completam o mínimo | em curso |
| κ baixo (<0.6) na fronteira BAL×STREAM | média | rótulos fracos | investigar rubricas ANTES de treinar; adjudicação; excluir ambíguos | gate M1 |
| Professores piores que o relatório no dev comum | média | destilação inútil | teacher_registry por métrica dev; gold só com argmax concordante; ablação A/B controla | previsto |
| Catastrophic forgetting (STREAM apaga planning/caps) | alta | gate falha | replay 40%+ das linhas; gates por época no dev; precedência de escolha | previsto |
| FPR BAL→STREAM alto (83 FPs do T903) | alta | gate STREAM P | contrastivos com hard negatives; gate P ≥ 0.75 pré-registrado | previsto |
| Teto 20 CU estourado | baixo | campanha parada | usage por fase; parar+relatar; braços restantes documentados | monitor |
| Vazamento de sealeds históricos | baixa | validade | proibição + auditoria local + construção nova; limitação documentada (sealeds não estão no disco) | mitigado |
| Sem golden humano | certa | claims | nada é chamado de golden; tiers silver documentados; limitation no decision.json | aceito |
| Sobrescrever artefatos T900–T903 | baixa | irreversível | escrever só em t904/ e ~/.cache; re-hash dos 9 pesos no fim | planejado |
| E2E .61 conflita com geração (GPU única) | certa se simultâneo | hang | sequência estrita: geração → labels → E2E | ordem definida |
