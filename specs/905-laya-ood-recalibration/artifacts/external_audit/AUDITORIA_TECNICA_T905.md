# Auditoria independente — Laya T905

**Data da auditoria:** 2026-10-25  
**Fontes fornecidas:** `REPORT_T905.md`, `sealed_b0.json`, `sealed_n2.json`, `eval_t905_{b0,n1,n2}.json`, `human_review_batch.json`.  
**Escopo:** reconciliação das métricas agregadas, análises de confusão, auditoria exploratória de rótulos e avaliação dos critérios de promoção. Não foram fornecidos pesos, manifestos completos, observações preditas por exemplo, logs E2E ou datasets de train/dev/sealed em bruto.

## Síntese executiva

- O candidato **N2** supera **B0** em todas as métricas agregadas listadas no sealed T905, mas passa somente **5/8 gates**. Sua promoção permanece bloqueada conforme a política congelada.
- Recalculados das matrizes QoS: acurácia B0 **287/450 = 63,78%**, N2 **358/450 = 79,56%**; Macro-F1 B0 **0,65954**, N2 **0,78353**. Os números divulgados conferem.
- Principal avanço: `QI_CRITICAL` recall de **91/188 (48,4%)** para **155/188 (82,45%)**; `QI_FAILOVER` de **52/75 (69,33%)** para **70/75 (93,33%)**.
- Regressão de classe: `QI_BALANCED` recall de **81/97 (83,51%)** para **66/97 (68,04%)**. O N2 prediz `QI_CRITICAL` para **21/97** BALANCED. Assim, a “dominância em tudo” vale somente para indicadores agregados selecionados.
- Streaming: **46/56** verdadeiros positivos em ambos, N2 reduz falsos positivos de 7 para 3; precisão 0,8679 → 0,9388.
- `cap__rust_refactor` e `cap__python_refactor` apresentam F1 **0** em B0 e N2, tanto em dev quanto no sealed. No lote de revisão humana, os **300** rótulos para cada uma dessas capacidades são **False**. Sem amostras positivas adequadas, a métrica é inadequada para concluir competência; se ambas as F1 permanecerem zero, o teto matemático de média Macro-F1 das oito capacidades é **0,75**, inferior ao gate 0,85.
- O lote de revisão tem discrepâncias semânticas que impedem tratá-lo como golden já revisado. Gerado CSV separado de triagem automatizada para inspeção humana.

## Comparação dos resultados — sealed n=450

| Métrica | B0 | N2 | Diferença N2-B0 | Gate |
|---|---:|---:|---:|---:|
| QoS Macro-F1 | 0,6595 | 0,7835 | +0,1240 | ≥0,85: falha |
| QoS accuracy | 0,6378 | 0,7956 | +0,1578 | — |
| STREAM precision | 0,8679 | 0,9388 | +0,0709 | ≥0,75: passa |
| STREAM recall | 0,8214 | 0,8214 | 0 | ≥0,80: passa |
| STREAM F1 | 0,8440 | 0,8762 | +0,0322 | ≥0,77: passa |
| Urgency Macro-F1 | 0,5609 | 0,9067 | +0,3458 | ≥0,85: passa |
| Planning accuracy | 0,7378 | 0,9333 | +0,1955 | ≥0,95: falha |
| Complexity Macro-F1 | 0,2586 | 0,8147 | +0,5561 | ≥0,70: passa |
| Caps média Macro-F1 | 0,5810 | 0,6755 | +0,0945 | ≥0,85: falha |
| Latência p50 de avaliação | 132,1 ms | 125,4 ms | −6,7 ms | guardrail sem regressão |

Os CIs pareados / testes de não inferioridade **não podem ser recalculados** a partir de matrizes agregadas: é preciso preservar as predições por exemplo dos dois modelos sobre as mesmas entradas.

**Intervalos Wilson binomiais exploratórios, não pareados (95%):** acurácia QoS B0 0,592–0,681; N2 0,756–0,830; recall STREAM N2 0,702–0,900. O recall STREAM de 0,821 atende ao gate pontual, mas a amostra de 56 positivos ainda produz incerteza considerável.

## Erros por classe de QoS

| Classe | Suporte | B0 corretos | N2 corretos | Variação recall (pp) |
|---|---:|---:|---:|---:|
| BALANCED | 97 | 81 | 66 | −15,46 |
| CRITICAL | 188 | 91 | 155 | +34,05 |
| STREAM | 56 | 46 | 46 | 0 |
| LOW_COST | 34 | 17 | 21 | +11,76 |
| FAILOVER | 75 | 52 | 70 | +24,00 |

O F1 do BALANCED melhora de 0,5329 para 0,6633 **apesar** da queda em recall, pois a precisão aumentou de 0,3913 para 0,6471. Exigir tanto performance macro quanto restrições de custo de erro por classe no próximo gate.

## Capacidades (ponto crítico)

| Capacidade | B0 F1 | N2 F1 | B0 Recall | N2 Recall |
|---|---:|---:|---:|---:|
| rust_refactor | 0 | 0 | 0 | 0 |
| python_refactor | 0 | 0 | 0 | 0 |
| unit_testing | 0,916 | 0,980 | 0,961 | 0,961 |
| security_audit | 0,943 | 0,976 | 0,976 | 0,964 |
| code_review | 0,725 | 0,966 | 0,986 | 0,986 |
| research | 0,933 | 0,992 | 0,952 | 0,984 |
| summarization | 0,575 | 0,733 | 0,939 | 0,643 |
| translation | 0,557 | 0,757 | 0,916 | 0,771 |

Há uma **regressão de recall** em `summarization` e `translation`. Inspecionar suporte positivo/negativo, TN/FP/FN, limiar por pergunta e distribuição multilíngue. O sealed só fornece F1, precisão e recall, mas não suporte/contagens dessas oito perguntas. Sem denominadores não se pode distinguir rótulo ausente de falha real da cabeça classificadora. O lote de revisão humana tem 0 positivos para refatoração Rust/Python, impossibilitando corrigir essa lacuna se não forem criados exemplos positivos.

## Auditoria do lote humano — 300 exemplos

- **300 IDs e 300 textos únicos; 116 famílias; 150 PT e 150 EN.** Classes QoS sugeridas artificiosamente balanceadas: 60 por classe. `stratum` de origem: balanced 96, failover 65, stream 62, lowcost 39, critical 38.
- **76/300 (25,3%)** possuem `stratum` que não corresponde à classe sugerida em `labels_para_conferir.qos_profile`. `stratum` pode ser metadado de geração e não verdade-terreno: tratar como **sinal para revisão**, jamais como prova automática de erro.
- **82** exemplos referenciam termos de tradução em inglês/português com `cap__translation=False`, e **74** termos de resumo/sumarização com `cap__summarization=False`, por heurísticas léxicas simples. *Essas são filas candidatas, não 82/74 erros confirmados.*
- **Exemplos nítidos para revisão:** `hr-001` pede tradução de documentação e tem `cap__translation=False`; `hr-005` exige tradução de UI e tem `cap__translation=False`; `hr-020` solicita enviar cada resultado assim que terminar, mas `qos_profile=QI_BALANCED` apesar de `stratum=stream`; `hr-003` pede pesquisar e então limpar os arquivos, porém está rotulado como `needs_planning=False`; `hr-031` pede traduzir no fluxo, mas `cap__translation=False`.
- Algumas mensagens contêm sintaxe artificial de interface (`unit_testing`, `code_review`, `security_audit`, `translate`) que pode servir como atalho lexical. Balancear esses dados com pedidos naturais sem nomes de ferramentas e com variantes PT/EN, e testar essa diferença.
- A revisão deve ser **cega às labels e a stratum**, com pelo menos dois anotadores em amostras de controle e adjudicação; não treinar nos rótulos atuais como se fossem ouro humano.

Arquivo de triagem anexo: `T905_TRIAGEM_REVISAO_HUMANA.csv`, preserva os 300 exemplos com flags automáticas e colunas vazias para resultado humano.

## Lacunas de método e rastreabilidade

1. O relatório declara 5.505 linhas no pool e splits 4.802+130+111+450 = **5.493**. É necessário reconciliar **12 registros** omitidos nos splits ou documentar a filtragem. Declarados 2.044 textos medidos versus 1.965 integrados: reconciliar 79 registros excluídos/não utilizados.
2. Concordância κ=1,0 de dois LLMs em 201 exemplos não é revisão humana independente e não prova acerto em QoS; verificar prompts dos julgadores e inspeção humana.
3. Sem predições por exemplo, não é possível reproduzir bootstrap pareado, McNemar, não inferioridade 6/6 e análise de caso. Solicitar `predictions_sealed_b0`, `predictions_sealed_n2`, protocolo pré-registrado e hashes dos dados.
4. O relatório do shadow menciona decisão aceita com `shadow_only=false` junto de `tasks_seen=tasks_written=0`. Verificar a semântica do campo e qual barreira efetivamente impediu materialização. A ausência de tarefas não prova, por si, que a flag estivesse configurada no modo esperado.
5. E2E teve apenas 20 dos 32 runs de latência concluídos após OOM-killer; solicitar dados por rodada, definição/unidade de p50 e causas de exclusão; latência E2E não é diretamente comparável com 125 ms do benchmark isolado do classificador. Preservar janelas de medição idênticas.
6. A calibração `d43d90c1…` foi aplicada com 111 pares, mas não foram fornecidas ECE/Brier por pergunta, curvas de confiabilidade, ou contagens por perfil no conjunto de calibração. Não se pode validar calibração exclusivamente pelos JSONs anexados.
7. O B0 "vigente" neste relatório tem prefixo de SHA `bda679e7…`, diferente do checkpoint histórico T900 (`35813054…`). Registrar a linhagem do B0/T904 e os manifests; não presumir que o B0 é idêntico ao T900.
8. Sem `model.safetensors`, manifestos completos e logs de execução, não é possível verificar pesos, compatibilidade `laya.load()` e integração ativa do checkpoint N2 diretamente.

## Recomendação e próximo experimento

**Manter o B0 vigente** como baseline operacional até completar os gates pré-registrados e uma nova avaliação com anotações humanas. **Preservar o N2** como candidato experimental para shadow/ablação; não descartá-lo: as melhorias em CRITICAL, FAILOVER, urgência e complexidade são importantes.

A próxima rodada deve priorizar correção/avaliação de `cap__rust_refactor` e `cap__python_refactor` (com suporte positivo comprovado), revisão humana cega de QoS, maior precisão de `QI_BALANCED` e manutenção de CRITICAL/STREAM/FAILOVER. O controle do próximo teste deve incluir casos contrastivos reais e execução DDS E2E completa, com OOM solucionado. Usar novo sealed que não se torne conjunto de ajuste e exigir regressões máximas por classe, não só Macro-F1.
