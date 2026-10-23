# experiment_plan — T904 Unified

## Hipóteses (do prompt, pré-registradas)

H1 consolidação (estudante único + replay ≥ competências selecionadas sem custo online) ·
H2 destilação seletiva > hard-label no mesmo dataset · H3 replay evita regressão de
planning/complexity/caps sob foco STREAM · H4 contrastivos genuínos melhoram P **e** R de
STREAM OOD · H5 ganho só vale se atravessar worker→DDS→validator sem regressão.

## Braços (mesma infraestrutura, mesmo dev, mesmas linhas onde comparável)

| Braço | Init | Dados | Pergunta | Status |
|---|---|---|---|---|
| B0 | T900 | — (eval) | baseline vigente | planejado |
| B1 | T902-B | — (eval) | urgência/QoS | planejado |
| B2 | T903-C | — (eval) | STREAM | planejado |
| A | T900 | train_nodistill (hard) | ganho dos dados isolado | planejado |
| B | T903-C | train_nodistill (hard) | init STREAM ajuda? | planejado |
| C | T900 | train (gold destilação) | efeito KD (controle A) | planejado |
| D | T903-C | train (gold destilação) | **candidato Unified** | planejado |
| E | — | LwF/EWC | anti-esquecimento | NOT_RUN (SDK sem suporte) |
| F | — | fusão/SWA/TIES | consolidação por pesos | NOT_RUN (exploratório, desencorajado) |

Hiperparâmetros idênticos aos treinos T902/T903 (epochs 4, micro-batch 8, grad-accum 8,
encoder_lr 2.5e-5, head_lr 1e-4, loss soft-ce, seeds 904). `checkpoint_latest` por época.
Early stopping por gates de regressão no dev a cada época (QoS F1, STREAM P/R, planning,
complexity, caps) — gravado em `regression_gates.json`.

## Métricas (M6, por qid/classe no dev)

Confusão 5×5, per-class P/R/F1, FPR BAL→STREAM, STREAM P/R em anti-keyword/negação/famílias
inéditas, needs_planning acc + Brier/ECE P(true), complexity/urgency MAE + within-one,
caps F1 por capability (não só média), latência p50/p95/p99, calibração separada
choice/noul no split calibration (temperatura NUNCA no sealed).

## Seleção e sealed

1. Escolher UM finalista no dev (precedência: integridade → fidelidade contratual →
   regressões → QoS F1 + STREAM P/R → calibração/latência).
2. Congelar: SHA do checkpoint, calibração, thresholds, scripts → `decision_plan_presealed.json`.
3. Abrir sealed T904 UMA vez para finalista + B0 + B1 + B2 fixos. Falhou? Sem re-tentativa
   (novo teste exigiria T905 com dados novos).
4. Status: KEEP_T900 / T904_OFFLINE_ELIGIBLE_AWAITING_APPROVAL / BLOCKED_WITH_TRAINED_CANDIDATE.

## Estatística

ICs pareados por família (bootstrap por cluster familiar) quando houver clusters suficientes;
caso contrário declarar IC não confiável (sem bootstrap por linha mascarando dependência).
Não-inferioridade vs T900 pré-definida no dev por pergunta.
