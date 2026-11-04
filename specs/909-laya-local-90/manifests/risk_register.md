# Registro de riscos T909
| Risco | Prob | Impacto | Mitigação |
|---|---|---|---|
| ROCm instável em treino longo | média | braço perdido | checkpoint/época; retomar braço na .61 (provido) |
| Térmica .61 re-degrada | média | geração lenta | master térmico de rajadas (T908); gen noturna |
| Yield do corpus < quotas | média | sealed menor | reduzir N ao maior quota-feasível; INSUFFICIENT_POWER no IC |
| Rótulos cx ainda acoplados a comprimento | média | gate cx falha | eixo S-C com hard-neg de comprimento + filtro v6 |
| STREAM regride de novo vs C2 | média | gate regressão | braço B (init C2) + hard-negs stream↔paralelismo |
| Vazamento corpus↔sealed | baixa | invalidez | família=lote + 8-gram bidirecional + near-dup |
| Divergência CPU/ROCm numérica | baixa | bloqueio promoção | paridade M9 com export de probs |
| Falso "review=security" (achado M0) | alta | cap security falha | hard-positives S-K com wording review+vulnerability |
