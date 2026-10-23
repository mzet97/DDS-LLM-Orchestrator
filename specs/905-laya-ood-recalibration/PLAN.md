# T905 — Recalibração OOD dos gates Laya (PLANO — não executado)

> Estado: PLANEJADO (2026-10-22). Nada executado. Sucessor natural da T904 Unified,
> cujo sealed novo (567 casos OOD) revelou que os gates absolutos herdavam alvos de
> holdouts SINTÉTICOS — no OOD real nem o baseline T900 os atinge (planning 0.302).
> Candidato vigente: arm_C `bda679e7…` + calib `6e1543f3…` (PROMOVIDO, ver
> `904-laya-unified-multiteacher/PROMOTION.md`).

## Objetivo

Produzir gates **estatisticamente dimensionados para OOD real** e um novo teste
confirmatório (sealed T905) que decida, com ICs, se o vigente permanece ou se um novo
ciclo de treino (com dados novos) o substitui — sem reutilizar qualquer sealed anterior.

## Linhas de trabalho

### 1. Dados (a causa-raiz é dado, não treino)
- **Real-text OOD bank novo**: 1.500–2.500 candidatos INÉDITOS, fontes variadas
  (traces de produção do orquestrador + SPROUT v5 com outro split temporal + domains
  não cobertos). Proibido: frases/paráfrases de sealed T902/T903/T904.
- **Urgency por prazo medido**: contrastivos com deadlines explícitos extraídos/normalizados
  (o gate 0.746 falhou por rótulo fraco — T901 já diagnosticou).
- **Caps reais**: hoje sub-representados em texto real (0.767). Coletar pedidos reais com
  caps rotulados por execução (qual capability o agente de fato invocou).
- **Complexity medido**: nº de etapas dependente MEDIDO do gabarito do cenário, não inferido.
- **Tier humano**: revisão humana de um golden ≥300 (mesmo que parcial) — pré-requisito
  para chamar os novos gates de confiáveis.

### 2. Gates recalibrados (pré-registro ANTES de qualquer treino)
Dimensionar n por classe para ICs alvo (±0.05 em recall STREAM com prevalência 25%)
e fixar os novos thresholds sobre a DISTRIBUIÇÃO REAL (referência: melhor modelo atual
no sealed 567 + margem):
- qos_macro_f1 ≥ 0.85 **mantém** (arm_C chegou a 0.823 — gap real de 0.027)
- stream P ≥ 0.75 / R ≥ 0.80 / F1 ≥ 0.77 **mantêm** (atingidos: 0.936/0.903/0.919)
- urgency_macro_f1 ≥ **0.85** (era 0.90 sem base empírica; melhor absoluto hoje 0.746)
- planning_acc ≥ **0.95** (era 0.98 sintético; 0.926 hoje com dados melhores)
- complexity_macro_f1 ≥ **0.70** (com rótulos medidos; 0.529 hoje sem eles)
- caps_macro_f1 ≥ **0.85** (com caps reais; 0.767 hoje)
- não-inferioridade vs arm_C em TODAS as perguntas (margem −0.02, IC pareado por família)
- latência p50 ≤ +10% vs vigente; integridade worker/DDS/Rust inalterada

### 3. Treino (só se os dados novos justificarem)
Mesma receita do vencedor da T904 (init vigente + destilação + replay + contrastivos,
soft-ce, seeds múltiplas ≥2) com ablação controlada; checkpoint por época; orçamento
T4 ≈ 6–10 CU.

### 4. Protocolo de decisão (idêntico ao da T904, já provado)
Freeze do finalista + thresholds → sealed T905 aberto 1× → gates →
`KEEP_VIGENTE | T905_OFFLINE_ELIGIBLE_AWAITING_APPROVAL | BLOCKED_WITH_TRAINED_CANDIDATE`.
Shadow DDS + E2E pareado obrigatórios antes de qualquer promoção (promoção sempre por
decisão do autor).

## Orçamento estimado
Geração/rotulagem .61 ~3–5 h · Colab ~6–10 CU · revisão humana (autor) ~2–4 h do seu tempo.

## Pré-condições
1. Commit/push da T904 (feito nesta rodada). 2. Aprovação do autor no plano e nos gates
pré-registrados. 3. Fonte de golden humano definida (o próprio autor ou revisor delegado).
