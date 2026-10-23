# labeling_policy_v3 — T904 Unified (M1)

## Fronteira central: QI_BALANCED × QI_STREAM_LIKE

- **QI_STREAM_LIKE** somente quando o texto pede **receber resultados intermediários utilizáveis conforme produzidos** (entregas parciais/contínuas: "envie cada análise assim que terminar", "versões parciais do relatório durante o processamento").
- **QI_BALANCED** quando a entrega é o **resultado completo ao final** — mesmo com: barra/percentual de progresso; execução paralela; palavras "rápido", "assíncrono", "em tempo real"; **hard negatives temáticos** (Kafka Streams, SSE, stream processing, stdout, stream de logs, streaming de vídeo como ASSUNTO do trabalho, não como modo de entrega); e negações explícitas ("não envie partes").
- Deadline/prazo afeta **urgency** (evidence-based), nunca transforma qos por palavra-chave. "Urgente" sem prazo → urgency evidenciada pelo texto; qos segue os outros requisitos dominantes.
- Demais perfis: LOW_COST (batch eficiente, sem sensibilidade), CRITICAL (prazo duro/confiabilidade), FAILOVER (resiliência a recursos indisponíveis) — separados da fronteira STREAM.

## Procedimento (evidência antes de rótulo)

1. **GEN**: textos gerados por cenário (domínio × expectativa de entrega × prazo × complexidade × caps × idioma) SEM rótulo prévio anexado ao texto; cenário serve só de estrato, nunca de gabarito.
2. **LABEL_A** (temperatura 0, rubrica política): rótulos dos 12 qids + evidência (trecho ≤200 chars) + confiança.
3. **LABEL_B** (rubrica redigida diferente, temperatura 0.2) sobre **subamostragem estratificada 25%** (delivery×lang): dupla anotação independente para estimar acordo (κ em qos_profile; reportado em `reports/annotation_agreement.json`).
4. **Adjudicação determinística** das divergências do subconjunto (regras da política: p.ex. termo "stream" como assunto + sem pedido de parcial ⇒ BALANCED); casos ainda ambíguos → status `review` (fora do treino).
5. Tiers: `silver` (LABEL_A; silver-dual quando LABEL_B concorda), `template-hardneg` (determinístico), `review` (excluído do treino), `reused-reviewed` (t901 270). **Nenhum dado é chamado de golden humano** — não há revisão humana nesta campanha (limitação registrada; o sealed T904 herda tier silver).

## Aceite M1

Cada rótulo em treino/dev/sealed tem justificativa extraível (campo `evidence`), sem contradição óbvia com a política; discordâncias do subconjunto duplo resolvidas por regra ou excluídas; κ reportado honestamente (κ < 0.6 em qos ⇒ investigar antes de treinar, não calar).
