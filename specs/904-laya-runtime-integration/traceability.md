# T904 — Rastreabilidade (requisito → código → teste → evidência)

| Req | Código | Teste | Evidência |
|---|---|---|---|
| REQ-904-001 | `adapter.py` (`checkpoint_path`), `cli.py` (`--checkpoint-path`) | T-904-M2-01 | REPORT M2/M3; smoke M3 |
| REQ-904-002 | `adapter.py` (hash pré-load; erros tipados) | T-904-M2-02/03/04 | REPORT M2; smoke hash divergente |
| REQ-904-003 | `adapter.py` (offline envs; path para `laya.load`) | T-904-M2-05 | REPORT M2 |
| REQ-904-004 | `adapter.py` (`model_id`/`checkpoint_revision`), `decide.py` (fluxo já existente) | T-904-M2-01/06 | shadow M5 (decisão capturada) |
| REQ-904-005 | `adapter.py` (`calibration_path`→pin); `evidence/calibration_manifest.md` | T-904-M2-08 | manifesto M4 |
| REQ-904-006 | `space.py` (inalterado) | existente + shadow | `evidence/m5-*.json` (tasks_seen=0) |
| REQ-904-007 | fluxo de gates (plan.md M6) | — | REPORT M6 (verde ou BLOCKED) |
| REQ-904-008 | — (não-ação) | — | registry; hash re-conferido pós-execução (M7) |
| REQ-904-009 | — (não-ação) | cargo/pytest existentes | REPORT M7 |
| REQ-904-010 | docs | — | REPORT/REPRODUCE/FINAL_HANDOFF |

Rastreabilidade herdada (não duplicar): fase 900 ⇒ `specs/900-laya-adaptive-routing/traceability.md`;
decisões científicas T901–T903 ⇒ `laya_t900_handoff/{t901,t902,t903}/`.
