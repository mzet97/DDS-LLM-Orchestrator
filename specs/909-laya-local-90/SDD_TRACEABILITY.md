# SDD_TRACEABILITY — T909 (Laya Unified 0.90, local-only)

Mapa fase → artefato → status (VERIFIED / REPORTED / NOT_RUN / BLOCKED / PENDING).
Atualizado ao longo da campanha; fonte canônica de cada claim.

| Fase | Requisito | Artefato | Status |
|---|---|---|---|
| M0 | Inventário SHAs (6 checkpoints + calib T908) | `audit/sha_manifest.json` | VERIFIED (6/6 batem com PROMPT_MESTRE) |
| M0 | `laya.load()` T908-a/C2/B0 | `audit/BASELINE_INVENTORY.json` | VERIFIED (CPU, load 7.7-12.5 s; smoke revelou fronteira security_audit×code_review) |
| M0 | Reproduzir métricas T908 ±0.001 | `audit/REPRODUCTION_CHECK.json` | VERIFIED — **GATE_M0 PASS (20/20)** |
| M0 | Atlas de erros T908 v3 | `audit/T908_ERROR_ATLAS.md` + `per_question_metrics.csv` | VERIFIED (n=590 × 4 modelos) |
| M0 | Paridade de decodificação score | `audit/score_decoding_parity.md` | VERIFIED (evaluator=argmax L72-85; worker=EV decide.py:184; SDK expõe ambos) |
| M0b | Triagem causa-raiz (SILVER) | `audit/triage_m0b.jsonl` + `_summary.json` | VERIFIED (n=105 na .61: urg 0→1 = 91% MODEL_ERROR; cx 2→1 = 100% MODEL_ERROR; CRIT→BAL = 24% LABEL_ERROR) |
| M1 | ROCm na RX (gfx1100) | `infra/SMOKE_TRAINING.json` | VERIFIED (torch 2.9.1+rocm6.4; treino real 1 época PASS; reload `laya.load()` PASS 13,2 s) |
| M1 | CUDA na .61 | `infra/HOSTS_INVENTORY.md` | VERIFIED (venv-t908 cu126 CUDA True; gemma :8083 200; up 26h) |
| M1 | Decisão de host | `infra/HOST_DECISION.md` | VERIFIED (RX = treino líder no distrobox; .61 = geração/reserva; confirmação do autor registrada) |
| M2 | Rubrica v6 + proveniência | `labels/LABEL_POLICY_T909.md`, `PROVENANCE_SCHEMA.json`, `HUMAN_REVIEW_PROTOCOL.md`, `adjudication_log.jsonl` (vazio) | VERIFIED; revisão humana PENDING ⇒ selo `NOT_HUMAN_VALIDATED` |
| M3 | Ablação de decoder no DEV | `evaluation/DECODER_ABLATION_DEV.md` + `decision_to_train.md` | PENDING (aguarda DEV T909 gerado; evaluator extendido já persiste todas as distribuições — validado em dry-run) |
| M4 | Corpus dirigido 5 eixos × 40 domínios | `/tmp/laya_t909/gen_*.jsonl` (.61) + fetch local | EM CURSO (S-U concluído 319 lotes/957 textos; S-C/S-Q/S-K/S-P encadeados; drops de marcador = 0 até o momento) |
| M5 | Splits + sealed NOT_OPENED | `datasets/splits/`, `datasets/sealed_t909/` | PENDING (script validado em dry-run: guard de soma de quotas capturou o caso T908-v2; família = lote) |
| M6 | Pré-registro antes do treino | `specs/909-laya-local-90/manifests/gates_preregistration.json` + EXPERIMENT_PLAN/risk/ROLLBACK | VERIFIED (commitado e39f7c3→[T-909-2] antes de qualquer treino T909) |
| M7 | Treino braços A/B (RX) | `models/arm_a`, `models/arm_b` | NOT_RUN (aguarda corpus; lançador `train_t909_rx.sh` pronto, checkpoints em NVMe) |
| M8 | Dev eval único + seleção min-F1 | `evaluation/SELECTED_FINALIST.json` | PENDING (script m8_select_t909.py validado) |
| M9 | Calibração + paridade CPU/ROCm/CUDA | `calibration/calibration_v1.json`, `evaluation/device_parity_*.json` | PENDING (scripts prontos; CPU-only do venv-t908 local confirmado) |
| M10 | Abertura única sealed + GATES_FINAL IC95 | `sealed/SEALED_OPEN_PROTOCOL.json`, `sealed/GATES_FINAL.json` | PENDING (script m10 com contador anti-reabertura + bootstrap por família validado em dry-run) |
| M11 | Shadow DDS (pin calibrador T909) | `integration/SHADOW_DDS_REPORT.md` | NOT_RUN (harness T904 localizado: `run_shadow_t904.py`) |
| M12 | E2E A/B | `benchmarks/E2E_REPORT.md` | NOT_RUN |
| M13 | Entrega completa | `REPORT_T909.md`, `DECISION_T909.json`, MANIFESTs | PENDING |
| M14 | Política de promoção | DECISION (rollback T904 `bda679e7…`) | REGRA CONGELADA: nunca automática |

## Arquitetura respeitada

Laya Python = decisão semântica tipada; Rust = autoridade de validação/QoS-DDS/DAG/recuperação
(contrato `contracts.md`; worker usa esperança contínua para `score` por design). Nenhum novo
tópico DDS; `QUESTIONS_V1` intocada; fail-closed preservado; `QI_*` ≠ política DDS nativa.
