# Dataset card T902 (preencher pos-M2)
Origens: calib T900 (sintetico gemma4, licenca do projeto), contrastivos T901 (templates), novos cenarios T902 (templates novos, PT/EN). Sem dados pessoais. Biases: sintetico por construcao, templates limitados, STREAM historicamente ruidoso. Qualidade: politica rev.1, evidence_span, REVIEW excluidos, histogramas no M2.

## Piloto e escala (M1/M2 executados)
- Auditoria antiga: STREAM 235 exemplos, 191 (81.3%) sem cue de entrega no texto (checker por regex ampliado; limite: cue implicito pode escapar). urgency low 663, 58 (8.7%) com palavra urgente contraditoria.
- Limpeza por regra (nao por modelo): removeram-se 124 STREAM sem cue + 32 urg-low contraditorios do treino agrupado antigo (1440 -> 1284).
- Novos cenarios (templates disjuntos train/dev/sealed, frases distintas no sealed): train_new 792, dev_new 264, sealed_new 528. Checker: 0 STREAM sem cue (frases sealed "fluxo continuo" validadas semanticamente).
- Splits finais: train 2076 (SHA 9796703e...), focal 2889, dev 264, calibration 180, sealed 528 (SHA db457693..., lacrado). Overlap exato entre splits: 0.
