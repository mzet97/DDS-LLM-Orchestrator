# Auditoria M0 T903 (2026-10-20, read-only + smoke local)

## Matriz afirmacao -> evidencia -> veredito
| # | Afirmacao | Evidencia | Veredito |
|---|---|---|---|
| 1 | T900 intocavel SHA 35813054... | sha256sum local do model.safetensors = 35813054... (842609220 bytes) | CONFIRMADO |
| 2 | Arm B T902 disponivel (5d7dd652...) | model.safetensors local, SHA confere | CONFIRMADO |
| 3 | T902 oficial KEEP_T900 | rust/specs/902.../evidence/decision.json + laya_t900_handoff/t902/decision.json | CONFIRMADO |
| 4 | Numeros sealed T902 (QoS delta +0.407 IC[0.361,0.459]; stream 0.0->0.0417 meta 0.75 nao atingida) | sealed_result.json (results + paired_bootstrap) | CONFIRMADO |
| 5 | Sealed T902 ja aberto 1x, historico | sealed_result.json note + manifest_t902.json | CONFIRMADO: nao reutilizar |
| 6 | Old 450 e sealed 528 nao sao holdouts novos | Relatorios T900/T901 + decision.json T902 | CONFIRMADO: categoria historical_used |
| 7 | Datasets T902 preservados (/tmp/t902: train/train_focal/dev/cal/sealed 528) | ls + contagens verificadas | CONFIRMADO (workspace local; VM antiga perdida) |
| 8 | laya SDK + laya.load() funcionais | venv tese-laya, smoke? pendente re-run nesta VM | PENDENTE (M0 exec: smoke rapido CPU) |
| 9 | CLI 0.7.4, sem sessao ativa | colab version/sessions | CONFIRMADO (nenhuma sessao) |
| 10 | Conflito doc T901 urgencia-adversarial vs labeling_policy v2 | t901/RELATORIO_T901.md + 902/labeling_policy.md (trecho a citar: conflito explicito) | DIVERGENCIA REGISTRADA: resolver em M1 (texto do T901 vence para estilo, nao para deadline como atalho) |

## Lacunas
- Calibracao ECE/Brier de scores no sealed T902: verificar metodo antes de reusar (risco AC-08).
- Pesos Arm A/C T902 locais: verificar integridade (A: f8616426... conforme handoff; C: verificar SHA).
- Servidor LLM/planner para M9: verificar disponibilidade mais tarde.
