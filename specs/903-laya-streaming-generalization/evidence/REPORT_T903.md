# T903 — Relatório final: generalização QI_STREAM_LIKE

**Veredito: KEEP_T900** (T900 `35813054…` permanece vigente).
Sealed aberto 1x pós-freeze, SHA íntegro `7578942a…`. Sem tuning no sealed.

## Treinos (Colab `laya900`, GPU T4, `laya==0.4.10`)

| Braço | Perda | Seed | SHA-256 |
|---|---|---|---|
| A | RLCD limpo | 902 | `b2d9d950…` |
| B | RLCD focal | 902 | `ccb50b39…` |
| C (finalista) | soft-ce | 903 | `7de213c7…` |

Base somente leitura; T900/T901/T902 intactos. Checkpoints em
`laya_t900_handoff/t903/models/arm_{A,B,C}/model.safetensors`.

## Dev agrupado (150 itens; bootstrap por item 10k)

| Modelo | QoS acc / F1 | STREAM P / R | urgency F1 | IC95% QoS F1 |
|---|---|---|---|---|
| B0 (T900) | 0.62 / 0.545 | — / 0.0 | 0.31 | [0.41, 0.63] |
| A | 0.933 / 0.947 | 1.0 / 0.762 | 1.0 | [0.912, 0.977] |
| B | 0.94 / 0.950 | 1.0 / 0.786 | 0.979 | [0.913, 0.979] |
| C | 0.94 / 0.950 | 1.0 / 0.786 | 0.992 | [0.914, 0.979] |

Dev passa nas margens KEEP (stream≥0.75, QoS F1≥0.85, urg≥0.73,
planning/complex saturados). Exceção: `cap__research` 0.79 < 0.97 —
mas B0 tem 0.67 (probe fraco pré-existente, sem regressão; C melhorou +0.12).
Clusters dev são espelhos de template (erros idênticos por cenário) →
bootstrap por cluster degenerado; IC por item é o informativo.

## Sealed (506 itens, banco de frases D inédito)

| Modelo | QoS acc / F1 | STREAM P / R / F1 | urgency F1 |
|---|---|---|---|
| B0 | 0.591 / 0.544 | — / 0.0 | 0.298 |
| arm_C | 0.739 / 0.819 | 0.546 / 0.714 / 0.619 | 0.963 |

Gate: QoS F1 0.819 < 0.85; stream R 0.714 < 0.75 → **não promovido**.
Mecanismo: 83 BALANCED→STREAM (falsos positivos) + 40 STREAM→BALANCED,
uniformes nos 10 cenários sealed (9–16 erros/cenário) e 2/6 traps.
Os ganhos do dev não transferiram para o banco D: o modelo
super-dispara STREAM em formulações inéditas e perde ~29% dos STREAM reais.
B0 sealed ≈ B0 dev → metodologia consistente, sem vazamento.

## M8 / M9

- Shadow real (`run_shadow.py --domain 219`): **PASS** — 3 intents →
  3 ACCEPT shadow_only, `tasks_seen=0`, `tasks_written=0`.
- E2E LLM: **NOT_RUN** (sem pesos GGUF em cache; só `llama-server` sem modelo).

## Reproduzir

```bash
colab exec -s laya900 -f /tmp/vm_eval903.py        # dev (requer dados na VM)
colab exec -s laya900 -f /tmp/vm_sealed_eval903.py  # sealed: abrir 1x, sem tuning
python3 bootstrap_m6b.py                            # CIs (laya_t900_handoff/t903/)
```

Evidências: `evidence/{decision.json,m6_bootstrap.json,shadow-run-t903.log}`;
relatórios: `laya_t900_handoff/t903/{dev,sealed}_*.json`.
