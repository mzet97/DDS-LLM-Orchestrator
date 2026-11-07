# ERRATA T911 — 2026-11-07

Correções ao REPORT_T911.md / DECISION_T911.json após auditoria da T912 (exploração de datasets e do contrato de perguntas). Os resultados centrais da campanha (resolução do teto ordinal do encoder pelo LLM) permanecem válidos; o que muda é a interpretação da coluna caps e a causa da ausência de raras no treino.

## E1 — Os braços T911 treinaram com 4 perguntas, não 5 (caps NÃO supervisionada)

`FastDecisionModel.build_dataset` (unsloth/models/decision.py:1759) cria um item POR pergunta e pula toda pergunta cujo nome não tem chave no `gold` (`skip("has no gold")`). No dataset T911 (`{state, questions: QUESTIONS_V1 com a pergunta agregada "capabilities", gold: 12 chaves SEM "capabilities"}`):

- a pergunta `capabilities` foi **pulada** no treino (não existe gold["capabilities"]);
- as 8 chaves `cap__*` do gold foram **ignoradas** (não existem perguntas com esses nomes).

Portanto os 4 braços (A0/A1/B0/B1) foram treinados com apenas qos_profile, needs_planning, complexity, urgency. Consequências:

1. **caps_agg 0.35-0.51 no sealed media uma pergunta nunca supervisionada** (comportamento zero-shot do Qwen3.5 + LoRA treinado só nos outros eixos). O número não mede aprendizado de caps; é evidência de transferência, não de treino.
2. A recomendação "caps fora do escopo T911, documentada pelo teto T909/T910" (REPORT §3.1 nota) estava incompleta: caps não estava fora do escopo por decisão, estava **acidentalmente fora do treino**.
3. Correção no REPORT_T911.md §2.2/§3.1 e DECISION_T911 (nota de errata); CHECKPOINT_LINEAGE permanece (SHAs corretos).

## E2 — Causa raiz da ausência de raras no train_t911 (refinamento do §6 do REPORT)

O REPORT §6 atribuiu a ausência de raras ao boost T910 perdido em `/tmp/laya_t910b/`. A auditoria da T912 refina:

- o `pool_t910_items.jsonl` (8.850, EM TREE) **contém** raras: STREAM 128, LOW_COST 195, FAILOVER 223, cx3 236, urg1 600, urg2 451;
- o `prepare_t911_dataset.py` particionou o pool por MATCH DE TEXTO contra os splits/sealeds antigos; as raras do pool coincidem majoritariamente com textos do `sealed_t909` (mesmos 2.306 textos do sealed T911) e foram parar na partição `sx` (sealed), não no treino;
- os 335 boost com gold no train_t910 ESTÃO no train_t911 (match de texto confirmado 335/336), mas seus golds qos são quase todos BALANCED/CRITICAL (o filtro de concordância da T910 só aprovava os que o label LLM confirmava; raras com gold por construção eram majoritariamente descartadas ou foram para dev/cal/sealed);
- o estoque de boost com gold raro (414 itens: 170 S-QOS, 131 S-URG, 113 S-CX3) está **dentro do `t909/datasets/sealed_t910/pool.jsonl`**, que é conjunto selado e NÃO pode ser usado para treino (regra da T912).

Lição mantida e reforçada: a dose de raras tem que ser **gerada de novo** e o censo por classe tem que ser gate fail-fast do prepare. A alegação "boost não entrou por fontes /tmp perdidas" fica corrigida para "a partição por match de texto contra sealeds antigos e o filtro de concordância deixaram o treino sem dose de raras; o pool em tree tinha raras, mas elas estavam locked em sealeds".

## E3 — Inconsistência do protocolo sealed_t910 (sanidade para a T912)

`t909/datasets/sealed_t910/SEALED_OPEN_PROTOCOL.json` diz `NOT_OPENED, open_count=0`, mas as métricas `t909/sealed/metrics_t910_{B0repro,B1,B3,B4,T909}.json` (n=2000) existem: o sealed T910 foi aberto na prática. Causa: `integrate_t910.py` reescreve o protocolo a cada execução. Ação T912: corrigir o protocolo para OPENED (open_count=1, data das métricas) para que o leak-check e a auditoria da T912 não se confundam; nenhum número muda.

## O que NÃO muda

- Resultados qos/cx/urg/plan do sealed T911 (4 braços, IC95): válidos (essas 4 perguntas foram treinadas e medidas corretamente).
- DECISION = BLOCKED_WITH_TRAINED_CANDIDATE e finalista A0/A0_cal: mantidos (a seleção foi por min-F1 global que era 0.0 de qos raras, não de caps).
- SHAs e LINEAGE: mantidos.
- Ablação P0/P1: conclusão de "sem vencedor consistente" continua válida para os 4 eixos treinados.
