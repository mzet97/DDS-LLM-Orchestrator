# REPORT_T905 — Recalibração OOD dos gates Laya (2026-10-24)

Execução do plano `specs/905-laya-ood-recalibration/PLAN.md` com a mesma disciplina SDD
da T904. Gates pré-registrados **congelados antes de qualquer treino**
(`specs/905-laya-ood-recalibration/gates_preregistration.json`, commitado).

## Resposta executiva

**Gates recalibrados: 5/8 PASS** (na T904 eram 3/8) — os rótulos **medidos por construção**
resolveram **urgency (0.907 ≥ 0.85)** e **complexity (0.815 ≥ 0.70)**, os dois piores gates.
**Não-inferioridade vs o vigente: 6/6 PASS com dominância em tudo** (urgency +0.35,
complexity +0.56, planning +0.20, qos +0.12). Restam **qos (gap 0.067), planning (gap 0.017,
marginal) e caps (gap 0.175 — o mais duro)**. Status: **BLOCKED_WITH_TRAINED_CANDIDATE** —
o candidato N2 `fa48606b…` fica entregue como experimental; o vigente `bda679e7…` permanece.

## Pipeline executado

| Fase | Resultado |
|---|---|
| M0/M1 | ✅ 2.044 textos com **ouros medidos por construção**: complexity = nº de estágios descritos (1-9), urgency = banco de prazos inequívocos, caps = requerimentos explícitos; qos = dual-LLM (κ=1.0, n=201 — viés LLM×LLM documentado) |
| M2 | ✅ pool **5.505 linhas** (novas medidas 1.965 + t901 270 + sintéticos 2.000 + v4 1.200 + hardnegs 70); **auditoria 8-gram excluiu 264 rows/18 famílias** que colidiam com v4_test ou com o sealed 567 da T904 (conservador) |
| M3 | ✅ splits por família: train 4.802 / dev 130 / cal 111 / **sealed 450** (CRIT 188, BAL 97, FAIL 75, LOW 34, STREAM 56), `4872d8c0…`, chmod 000; **T904 sealed re-hashado intacto** (`bf11db46…`) |
| M5 | ✅ 2 braços na T4 (init **vigente** `bda679e7…`, soft-ce, 4 épocas): **N1 seed 905** `4518d717…`, **N2 seed 906** `fa48606b…`; downloads SHA VM=local; incidentes: 1º N1 morreu (upload silencioso de dados — corrigido com verificação explícita) |
| M6 | ✅ dev (n=130): N1/N2 >> B0 em tudo (urgency 0.38→0.87, complexity 0.26→0.70/0.76, planning 0.69→0.91); seeds concordaram; **finalista N2** pela precedência (QoS F1 0.779, STREAM P 0.941); calibração pin `d43d90c1…` (111 pares) |
| M7 | ✅ freeze (`manifests/decision_plan_presealed.json`) → **sealed aberto 1×** → tabela abaixo |
| M8 | ✅ shadow DDS com N2+calibração: `t904-ok` aceito com **`shadow_only=false`**; forjados rejeitados fail-closed; **tasks_seen=0/tasks_written=0** |
| M9 | ✅ E2E pareado via .61: N2 qos 0.875 vs vigente 0.844; **latência p50 1.290 vs 1.478 ms** (N2 mais rápido); híbrido plan_validity 1.0; 20/32 runs de latência (OOM-killer local; agregado n=80/braço, documentado) |
| M10 | ✅ entrega: `models/arm_n2/` + montagem `~/.cache/tese-laya/checkpoints/laya-t905-n2` (NÃO promovido); `decision.json`; **lote de revisão humana 300 estratificados** (`reports/human_review_batch.json`) |

## Sealed T905 (n=450, abertura única; N2 com calibração `d43d90c1…`)

| Métrica | gate | N2 | B0 vigente | gate |
|---|---|---|---|---|
| QoS Macro-F1 | ≥0.85 | **0.783** | 0.659 | ❌ (gap 0.067) |
| STREAM P / R / F1 | ≥0.75/0.80/0.77 | **0.939 / 0.821 / 0.876** | 0.868/0.821/0.844 | ✅✅✅ |
| Urgency Macro-F1 | ≥0.85 | **0.907** | 0.561 | ✅ |
| Planning Accuracy | ≥0.95 | **0.933** | 0.738 | ❌ (gap 0.017) |
| Complexity Macro-F1 | ≥0.70 | **0.815** | 0.259 | ✅ |
| Caps Macro-F1 médio | ≥0.85 | **0.675** | 0.581 | ❌ (gap 0.175) |
| Latência p50 | sem regressão | 125 ms | 132 ms | ✅ |

## Lições e incidentes

- Upload silencioso falhou 1× (pipe `tail -0` engolindo erro) → verificação explícita pós-upload.
- E2E local morto 2× pelo OOM-killer (memória do PC) → agregação offline com n reduzido documentado.
- `pkill`/`ssh` self-kill, aliasing de dict, `str.replace` silencioso — herdados e tratados da T904.

## Custo

Colab: 350,72 → ~344 CU no fechamento do sealed (piso 340; ~7 CU no T905). Geração na .61 (~6 h).

## Próximos passos

1. **Autor**: revisar `reports/human_review_batch.json` (300 casos) → golden async.
2. Decidir: manter vigente + atacar caps/qos com dados de execução real (caps por invocação
   real do agente; qos por revisão) OU promover N2 (domina o vigente em tudo; 3 gates abertos).
3. T906 somente com novos dados e gates re-dimensionados após o golden humano.
