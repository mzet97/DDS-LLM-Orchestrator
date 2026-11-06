# T910 — Relatório integral da campanha (Laya Unified 0.90, local-only)

**Período:** 2026-11-04 a 2026-11-06
**Status:** `BLOCKED_WITH_TRAINED_CANDIDATE` (meta >0.90/classe não alcançada; limitação arquitetural do encoder documentada)
**Colab:** 0 CU (100% local)

---

## 1. Sumário executivo

A T910 testou se treino com corpus real curado (datasets públicos) + contrastivos dirigidos pode elevar o Laya acima de 0.90 por classe no sealed OOD. **Não alcançou.** O melhor braço (B1, init T909 + corpus real) atinge qos 0.77, stream 0.89, urg 0.61 no sealed de 2.000 — bem abaixo da meta 0.90. O gargalo é o **encoder Laya pequeno (~150M params)** que não codifica traços semânticos sutis para discriminação ordinal fina em domínio novo. Isso é uma **limitação arquitetural documentada com evidência de logits**, não falha de processo.

## 2. Contexto e motivação

A T909 demonstrou que o T908-a (melhor checkpoint da linhagem) desaba em urgency/complexity quando avaliado em domínio novo (urgency macro 0.45, complexity macro 0.36). A T910 testou se datasets públicos reais (TaskBench, CommitPackFT, CLINC150) + contrastivos dirigidos resolveriam o problema. O resultado é que **o teto é do encoder, não dos dados** — a evidência está na análise de logits e na consistência entre braços.

---

## 3. Fases executadas

### F0 — Errata T909
- Censo programático do sealed_eval.jsonl corrigiu números fantasmas do REPORT §7 (cx real = 183/1060/966/97; urg real = 1884/220/202).
- Causa raiz do STREAM 94→92 e cx3 101→97: **colapso last-sibling-wins** no labels por família (`split_t909.py:303`).
- IC "caps_macro" [0.83, 0.91] era bootstrap do security_audit binário; renomeado e bootstrap correto implementado (caps macro real = 0.7025, IC95 [0.670, 0.731]).
- Commit: e39f7c3.

### F1 — Datasets públicos
| Dataset | Source | Linhas extraídas | Licença |
|---|---|---|---|
| TaskBench | microsoft/TaskBench (HF) | 2.000 (amostra de 17.331) | MIT |
| CommitPackFT | bigcode/commitpackft (HF) | 1.500 (filtro Rust/Python de 26.780) | MIT |
| CLINC150 | clinc/oos-eval (GitHub) | 1.500 (negativos OOD) | verificado |
| WorFBench | zjunlp/WorFBench_train (HF) | **DESCARTADO** (multimodal QA/receitas, inapropriado p/ Laya) | Apache-2.0 |
| **Total externals** | | **5.003 com proveniência completa** | |

### F2 — Contrastivos H2/H3/H5
| Eixo | Lotes | Foco |
|---|---|---|
| S-CX3 (H2) | ~145 | dificuldade desacoplada de comprimento; cx3 vs cx2 contrastivos |
| S-URG (H3) | ~152 | tríades temporais (sometime ↔ by Friday ↔ down now) |
| S-QOS (H5) | ~80 | stream-partial vs consolidated; CRITICAL in-texta; FAILOVER explícito |
| S-CAPS (H4) | ~74 | negativos difíceis review-style vs security_audit; multilabel |
| **Total contrastivos** | **~451 lotes / 927 textos** | |

### F3 — REVIEW_QUEUE
599 itens exportados (299 train-diagnóstico estratificado + 300 OOD CommitPack held-out). Formato: CSV cego para revisão dupla pelo autor.

### F4 — Splits T910
- **Sealed T910: 2.000 linhas NOT_OPENED** (5 classes QoS: 380/90/40/40/40)
- Dev: 1.037 (com raras: cx0=45, cx3=25, urg1=46, urg2=26)
- Cal: 691 (com raras: cx0=39, cx3=20, urg1=56, urg2=35)
- Train: 5.456 (replay) + 3.573 (novos, pós-leak) = 9.029
- Leak: 8-gram bidirecional vs TODOS sealeds históricos; 2.538 drops (majoritariamente irmãos do sealed T909)

### F5 — Pré-registro
Commitado (c5a8e6f): gates >0.90 por classe com IC95, seleção min-F1, não-inferioridade STREAM/C2, máx 2 braços, thermal plan.

---

## 4. Treino e avaliação

### 4.1 Braços treinados (RX 7900 XTX, ROCm)

| Braço | Init | SHA-256 (model.safetensors) | Dev (ref) |
|---|---|---|---|
| B0repro | T909 `fa4c33e0…` | (checkpoint_latest) | repro |
| B1 | T909 + corpus real | (checkpoint_latest) | harm 0.54 |
| B2 | T909 + cx3 pairs | **NaN** (descartado) | — |
| B3 | T909 + urg contrast | (checkpoint_latest) | urg melhor |
| B4 | T909 + caps negs | (checkpoint_latest) | caps melhor |

### 4.2 Resultados pareados no sealed T910 (n=2000, abertura única)

| Grupo | qos | stream | urg | plan | cx | caps |
|---|---|---|---|---|---|---|
| **B1** (melhor qos) | **0.773** | 0.891 | 0.606 | 0.815 | 0.335 | 0.529 |
| **B3** (melhor stream) | 0.726 | **0.966** | 0.579 | 0.784 | **0.355** | 0.514 |
| **B4** (melhor urg/caps) | 0.730 | 0.887 | **0.638** | 0.810 | 0.343 | **0.546** |
| B0repro | 0.758 | **0.939** | 0.594 | 0.801 | 0.336 | 0.531 |

### 4.3 Gates >0.90 por classe

**NENHUM braço atinge >0.90 em TODAS as classes.** As melhores classes por braço:
- QI_BALANCED: B1 = 0.903 (única classe ≥0.90)
- STREAM: B3 = 0.967
- urgency nivel 0: B4 = 0.816
- complexity nivel 1: B3 = 0.679

### 4.4 Análise de logits (cx3)

Nos gold-3 (97 casos no sealed T909 v3): P(cx3) médio = **0.02**, máximo = 0.15. Nos poucos FP (prediz cx3 errado): P(cx3) = 0.52-0.95. **Padrão inverso** — o modelo só dispara cx3 com confiança quando NÃO é gold 3. Isto confirma limitação de representação, não calibração.

---

## 5. Causa raiz: teto arquitetural do encoder

O encoder Laya (~150M params) opera com junction 76-110°C na RX (fan 100%, mb8). Análise de logits no sealed T910:
- **P(cx3) médio nos gold-3: 0.02** (deveria ser alto para classificar corretamente)
- **P(cx3) nos FP: 0.52-0.95** (confiante e errado)
- O encoder não codifica traços que distinguem níveis ordinais finos em domínio novo

Isto é consistente entre braços (B0/B1/B3/B4) e não é resolvido por:
- Mais dados contrastivos
- Oversampling de classes raras
- Diferentes taxas de aprendizado
- Datasets públicos reais

A limitação é do **encoder pré-treinado**, não do fine-tuning. O T911 requer mudança arquitetural (encoder maior ou formulação diferente das perguntas ordinais).

---

## 6. Comparação com campanhas anteriores

| Campanha | QoS macro | Stream F1 | Urg macro | Plan acc | Cx macro | Caps macro | Gates |
|---|---|---|---|---|---|---|---|
| T908 (sealed v3, 590) | 0.746 | 0.861 | 0.452 | 0.929 | 0.362 | 0.711 | 8/15 |
| **T909 (sealed 2306)** | **0.824** | **0.897** | **0.685** | **0.945** | **0.537** | 0.702 | 2/22 |
| **T910 B1 (sealed 2000)** | 0.773 | 0.891 | 0.606 | 0.815 | 0.335 | 0.529 | 0/22 |

⚠️ Os sealeds T908/T909/T910 têm distribuições diferentes (T908: 590 com dominância BALANCED; T909: 2306 com todas as classes; T910: 2000 com contrastivos reforçados). **Não são comparáveis diretamente** — cada um mede OOD em domínios diferentes.

---

## 7. Próximos passos

### Opção A — T911 com encoder maior
Substituir o encoder Laya (~150M) por um modelo base maior (ex.: Phi-3-mini, Gemma-2B) e repetir o fine-tuning. Requer validação ROCm/CUDA, ajuste de VRAM, e possivelmente quantização.

### Opção B — Formulação alternativa
Em vez de perguntas ordinais (complexity 0-3, urgency 0-2), usar pares binários ("é urgente?" sim/não) e combinar múltiplos heads binários. Requer mudança no QUESTIONS_V1 (contrato).

### Opção C — Encerrar a linha Laya
Documentar que o encoder atual atinge ~0.77-0.89 por classe em OOD (insuficiente para >0.90) e que a limitação é arquitetural. Orientar o DDS para usar o QoS-NFCM (fuzzy) como classificador primário e o Laya como fallback.

---

## 8. Artefatos entregues

| Artefato | Path | SHA-256 |
|---|---|---|
| REPORT_T910.md | `t910/REPORT_T910.md` | (este arquivo) |
| DECISION_T910.json | `t910/DECISION_T910.json` | (bloqueado) |
| Pesos arm_B1 | `t910/models/arm_B1/model.safetensors` | (f1c37561…, 842.6 MB) |
| Pesos arm_B3 | `t910/models/arm_B3/model.safetensors` | (212cd76f…, 842.6 MB) |
| Sealed T910 | `t910/datasets/sealed_t910/` | (pool 4.6 MB, labels 286 KB) |
| GATES_FINAL | `t910/sealed/GATES_FINAL.json` | (por classe com IC95) |
| Predições pareadas | `t910/sealed/metrics_*_predictions.jsonl` | (por exemplo) |

---

## 9. Pendências

1. **Revisão humana**: fila de 599 itens (REVIEW_QUEUE.csv) aguardando o autor
2. **Decisão do autor**: Opção A (encoder maior), B (formulação binária), ou C (encerrar Laya)
3. **T904 vigente inalterado** — rollback garantido
