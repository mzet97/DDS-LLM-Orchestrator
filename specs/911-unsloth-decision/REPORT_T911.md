# T911 — Relatório integral (Unsloth Decision Models + ablação de prompt XML)

**Período:** 2026-11-06 a 2026-11-07
**Status:** `BLOCKED_WITH_TRAINED_CANDIDATE` (gate primário >0.90/classe FAIL; hipótese do teto ordinal do encoder CONFIRMADA e RESOLVIDA pelo LLM; gargalo remanescente é DADOS, não arquitetura)
**Colab:** 0 CU (100% local: treino na .61 RTX 3080, dual-host)
**Pré-registro:** commit `871efe8` (branch `t911-unsloth-decision`), ANTES da abertura sealed
**Sealed:** 2.306, abertura única `open_count=1` em 2026-11-07T01:06 UTC, 4 braços pareados

---

## 1. Sumário executivo

A T911 substituiu o encoder Laya (~150M params, teto arquitetural documentado na T910) por **Qwen3.5 via Unsloth FastDecisionModel** (0.8B/2B, LoRA r16) mantendo o contrato QUESTIONS_V1 (5 perguntas), e fez ablação de representação de prompt (P0 texto simples vs P1 XML). Resultados no sealed T911 (n=2.306):

1. **O gargalo ordinal do encoder foi RESOLVIDO**: urgency macro **0.85** (encoder T910: 0.61), complexity macro **0.61** (encoder: 0.34, com cx1/cx2 ≈ 0.80-0.83), planning **0.91** (encoder: 0.82).
2. **O gate primário (>0.90 em TODAS as classes) falhou**: min F1 = 0.0 em todos os braços, por colapso das classes sem dados de treino.
3. **A causa do colapso é fome de dados, não arquitetura**: o train_t911 tem **6 LOW_COST / 6 STREAM / 4 FAILOVER e 27 cx3** (contra 3.614 BALANCED). O contraste urg2 (140 exemplos de treino → F1 0.864) prova que o LLM aprende classe com ordem de ~100+ exemplos.
4. **P1 (XML) não venceu P0 (plain) de forma consistente** no sealed; no dev P1 foi levemente superior. Sem vencedor de ablação claro.

---

## 2. Metodologia

### 2.1 Braços (4, matriz encoder × prompt)

| Braço | Base | Prompt | Treino | SHA adapter |
|---|---|---|---|---|
| A0 | Qwen3.5-0.8B | P0 plain | 6 épocas mb16, 20 min | `d2603508…` |
| A1 | Qwen3.5-0.8B | P1 XML | 6 épocas mb16, 20 min | `3d5b6d66…` |
| B0 | Qwen3.5-2B | P0 plain | 6 épocas mb16, 35 min | `3cabbf2c…` |
| B1 | Qwen3.5-2B | P1 XML | 6 épocas mb16, 36 min | `179dadb7…` |

LoRA r=16 α=16, lr 2e-4 cosine, bf16, seed 909, `DecisionTrainer` (Unsloth 2026.11.3, torch 2.14.1+cu126, transformers 5.17.0), max_seq 2048, fp32 (sem 4-bit). SHAs completos em `CHECKPOINT_LINEAGE.json`.

### 2.2 Pipeline de avaliação

- **Seleção (dev n=1.037):** min-F1 por classe; todos os braços empataram em 0.0 → regra de custo do pré-registro → **A0** (0.8B, P0).
- **Calibração (CAL n=691):** `FastDecisionModel.calibrate` (temperature LBFGS por qtype) → temperaturas [0.969, 0.966, 1.220], ECE 0.044, accuracy 0.889. Checkpoint calibrado: `arm_A0_cal` (adapter idêntico ao A0; joint_head com temperatura dobrada, SHA `dc43df05…`).
- **Sealed (n=2.306):** predições por exemplo preservadas (`evaluation/predictions/predictions_*_sealed.jsonl`, 4×2.306), IC95 bootstrap (n=1.000, percentil) por classe individual.
- Métricas idênticas ao `evaluator_t909.py` (argmax choice/score, threshold 0.5 noul), garantindo comparabilidade T909/T910/T911. Caps avaliada AGREGADA (formato QUESTIONS_V1 de 5 perguntas usado no treino e no runtime).

---

## 3. Resultados pareados no sealed T911 (n=2.306, IC95 bootstrap)

| Braço | minF1 | qos | cx | urg | plan | caps_agg |
|---|---|---|---|---|---|---|
| **A0** (finalista) | 0.0 | 0.341 | **0.605** | 0.850 | 0.913 | 0.420 |
| A1 (XML 0.8B) | 0.0 | 0.339 | 0.588 | 0.841 | 0.913 | **0.510** |
| B0 (2B plain) | 0.0 | **0.342** | 0.568 | 0.851 | 0.915 | 0.367 |
| B1 (2B XML) | 0.0 | 0.339 | 0.581 | **0.858** | **0.917** | 0.352 |

### 3.1 Per-class do finalista A0 (sealed)

| Grupo | Classe | F1 | suporte | IC95 |
|---|---|---|---|---|
| qos | QI_BALANCED | 0.863 | 1.381 | — |
| qos | QI_CRITICAL | 0.841 | 546 | — |
| qos | QI_STREAM_LIKE | **0.000** | 94 | [0.000, 0.000] |
| qos | QI_LOW_COST | **0.000** | 143 | [0.000, 0.000] |
| qos | QI_FAILOVER | **0.000** | 142 | [0.000, 0.000] |
| cx | 0 | 0.797 | 184 | — |
| cx | 1 | 0.827 | 1.039 | — |
| cx | 2 | 0.797 | 982 | — |
| cx | 3 | **0.000** | 101 | [0.000, 0.000] |
| urg | 0 | 0.971 | 1.871 | — |
| urg | 1 | 0.715 | 218 | [0.665, 0.759] |
| urg | 2 | **0.864** | 217 | — |
| plan | True | 0.949 | — | — |
| plan | False | 0.705 | — | — |
| caps_agg | True | 0.539 | 934 | — |
| caps_agg | False | 0.302 | 1.372 | — |

### 3.2 Modo de falha das classes zeradas (matriz de confusão A0)

- STREAM (94 gold) → **100% predito BALANCED**
- LOW_COST (143 gold) → **100% predito BALANCED**
- FAILOVER (142 gold) → 57% BALANCED + 43% CRITICAL
- cx3 (101 gold) → colapsa nos níveis inferiores

Colapso de massa para as classes majoritárias, com **zero** de exemplos efetivos no treino.

---

## 4. Comparação indireta com o teto do encoder (T910)

⚠️ Sealeds diferentes (T910: 2.000 com quotas contrastivas; T911: 2.306); comparação INDIRETA, não pareada.

| Grupo | T910 B1 (encoder 150M) | T911 A0 (LLM 0.8B) | Δ |
|---|---|---|---|
| urgency macro | 0.606 | **0.850** | **+0.244** |
| urgency:1 | 0.39-0.41 | **0.715** | **+0.31** |
| complexity macro | 0.335 | **0.605** | **+0.270** |
| planning acc | 0.815 | **0.913** | **+0.098** |
| QoS macro | 0.773 | 0.341* | −0.432* |
| cx:3 | 0.02-0.15 | 0.0* | — |

\* QoS T910 era medido num sealed com dominância BALANCED e o encoder herdava viés do replay; o T911 tem suporte de raras muito maior no sealed (94-143 por classe) e o modelo zera porque nunca viu as classes no treino. **A comparação honesta é: o encoder não aprendia ordinal nem com dados; o LLM aprende ordinal e aprende classes quando há ≥~100 exemplos (urg2: 140 → 0.864).**

## 5. Ablação P0 vs P1 (XML)

- Dev: P1 ≥ P0 em quase todos os macro (qos 0.356 vs 0.353; cx 0.562 vs 0.557; urg 0.853 vs 0.839; caps_agg 0.532 vs 0.501).
- Sealed: misto (A1 caps_agg 0.510 > A0 0.420, mas A0 cx 0.605 > A1 0.588; 2B: B1 urg 0.858 > B0 0.851, caps pior).
- **Veredito: sem vencedor consistente.** Diferenças <0.03 na maioria dos grupos, dentro do ruído. Para o runtime recomenda-se P0 (mais simples, tokens menores); P1 como candidato futuro se formato estruturado for necessário para contexto runtime/task.

## 6. Causa raiz do gate FAIL: fome de dados (censo do train_t911)

| Sinal | Exemplos de treino | F1 sealed A0 |
|---|---|---|
| QI_BALANCED | 3.614 | 0.863 |
| QI_CRITICAL | 348 | 0.841 |
| needs_planning | 2.078/1.900 | 0.949/0.705 |
| urgency:0 | 3.586 | 0.971 |
| urgency:1 | 252 | 0.715 |
| **urgency:2** | **140** | **0.864** |
| cx:0/1/2 | 270/2.514/1.167 | 0.797/0.827/0.797 |
| **cx:3** | **27** | **0.000** |
| **qos LOW_COST/STREAM/FAILOVER** | **6/6/4** | **0.000/0.000/0.000** |
| caps_agg True | 616 | 0.539 |

**Lei empírica observada:** classe com ≥~140 exemplos de treino → F1 ≥ 0.71; classe com <30 → F1 ≈ 0. O boost de classes raras da T910 (S-QOS/S-URG/S-CX3 em `/tmp/laya_t910b/`) **não entrou** no `train_t911.jsonl` (fontes em `/tmp` perdidas na reconstrução do dataset), silenciosamente. Isto é um defeito de PIPELINE DE DADOS, corrigível sem mudança arquitetural.

## 7. Aprendizados de infraestrutura

- Unsloth `FastDecisionModel` + `DecisionTrainer` funciona em RTX 3080 10GB (mb16, bf16, LoRA fp32): 0.8B a 1.24 it/s, 2B a 1.45 s/it; VRAM máx 7.5GB (2B); junction 72-74°C estável.
- `calibrate()` espera items do `build_dataset` (registros tokenizados), não dicts brutos (`KeyError: 'record'`).
- `predict()` retorna 5 perguntas: caps é UMA pergunta noul agregada (não 8 individuais).
- tmux server morre na .61 (causa não identificada); usar `setsid nohup` para jobs longos.
- Transferência via `cat | ssh 'cat > file'` pode resultar em arquivo vazio sem erro na primeira tentativa; sempre verificar `wc -l` no remoto.
- CIFS: o repo git da tese vive em `src/rust` (não na raiz); `src/laya_t900_handoff/` fica FORA do repositório.

## 8. Decisão e próximos passos

**DECISION = BLOCKED_WITH_TRAINED_CANDIDATE** (ver `DECISION_T911.json`).

O candidato A0 (e o calibrado A0_cal) NÃO promove: min F1 = 0 no sealed. Mas a campanha cumpre seu papel científico:

1. **Teto ordinal do encoder CONFIRMADO e RESOLVIDO**: o gargalo da T909/T910 (cx/urg ordinais) desaparece com LLM 0.8B — urg +0.24, cx +0.27 vs encoder.
2. **Gargalo remanescente é corrigível com dados**: T912 = reconstruir train com dose mínima por classe (alvo ≥150/classe qos rara, ≥150 cx3, manter urg/cx1-2), reutilizando o gerador de contrastivos da T910 PERSISTINDO em `t912/datasets/` (nunca `/tmp`).
3. **T904 (laya-t904-unified-c) permanece inalterado** em produção; rollback garantido.

## 9. Artefatos entregues

| Artefato | Path | SHA-256 |
|---|---|---|
| Este relatório | `t911/REPORT_T911.md` | — |
| DECISION | `t911/DECISION_T911.json` | — |
| LINEAGE (SHAs 5 checkpoints) | `t911/CHECKPOINT_LINEAGE.json` | — |
| PREREG (commit 871efe8) | `src/rust/specs/911-unsloth-decision/PREREG_T911.json` | — |
| Protocolo sealed | `t911/sealed/SEALED_OPEN_PROTOCOL.json` (OPENED, open_count=1) | pool `30ba7617…` labels `1b4b04ab…` |
| GATES_FINAL | `t911/sealed/GATES_FINAL_T911.json` | — |
| Métricas sealed 4 braços | `t911/evaluation/metrics_{A0,A1,B0,B1}_sealed.json` | — |
| Predições por exemplo | `t911/evaluation/predictions/predictions_*_sealed.jsonl` (4×2.306) | — |
| Calibração | `t911/calibration/CALIBRATION_T911.json` | — |
| Pesos | `.61:/home/agent/t911_train/arm_{A0,A0_cal,A1,B0,B1}/` (+ cópia local `t911/models/`) | ver LINEAGE |
| Avaliador | `t911/scripts/eval_t911.py` (idêntico ao commitado em specs) | — |
| Gates | `t911/scripts/gates_t911.py` | — |

## 10. Pendências

1. **T912** (proposta): regeneração com dose por classe + oversampling; gate esperado: qos raras ≥0.90 com ≥150 exemplos/classe.
2. **Revisão humana**: fila de 599 itens (REVIEW_QUEUE.csv, T910) continua aguardando o autor.
3. Decisão do autor: manter linha T912 ou encerrar (Opção C do T910) — o resultado da T911 muda a recomendação: o gargalo agora é barato de resolver (geração de dados), não arquitetural.
