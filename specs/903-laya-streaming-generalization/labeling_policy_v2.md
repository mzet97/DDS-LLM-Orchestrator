# Politica de rotulagem v2 (T903 rev.1) — DATA FIRST

## Precedencia de perfil (resolver conflitos nesta ordem)
1. Falha/resiliencia explicita -> QI_FAILOVER (ex.: continuar em outro recurso com retentativa).
2. Entrega incremental/interativa pedida ou implicada -> QI_STREAM_LIKE (acompanhar resultados parciais enquanto executa).
3. Prazo curto OU consequencia severa explicita -> QI_CRITICAL.
4. Lote sem urgencia com economia explicita -> QI_LOW_COST.
5. Caso contrario -> QI_BALANCED (padrao; silencio sobre entrega = resultado completo no fim).

## Regras operacionais
- Palavras "stream/streaming/Kafka/SSE/logs continuos/video" como TOPICO nunca rotulam STREAM; so o modo de entrega pedido.
- Negacao de parciais ("nao envie parciais", "so o resultado final") -> nunca STREAM, mesmo com outras cues.
- Escopo code ("faca streaming de dados no codigo, mas entregue so o relatorio final") -> topico, nao entrega.
- Deadline manda na urgencia (horas>dias>semanas), mas prazo curto sozinho nao cria STREAM; STREAM pode ser low urgency.
- Pedido indireto ("posso ir vendo?", "me mantenha atualizado conforme avanca") = STREAM se implicar resultados parciais; pergunta sobre status pronta sem pedir parciais = BALANCED.
- Ambiguo sem evidencia -> review/ambiguous/exclude na curadoria (sem classe ABSTAIN no contrato).
- Cada rotulo exige evidence_span (trecho observavel citado no registro de curadoria).

## Auditoria T903 (M1)
Taxa STREAM-sem-cue no dataset antigo: 81.3% (checker regex; caveat: implicito pode escapar).
Conflito T901/T902 resolvido: deadline e entrega passam a ser anotados como atributos independentes (modo_entrega, prazo) e o perfil derivado pela precedencia acima; urgência-adversarial do T901 vira categoria obrigatoria de dev (keyword traps).
