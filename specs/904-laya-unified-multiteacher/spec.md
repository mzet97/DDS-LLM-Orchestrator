# spec — T904 Unified Multi-Teacher (REQ-904U-*)

Fonte: PROMPT_MESTRE_T904_UNIFIED_LAYA_SDD.md + decisões do autor (2026-10-21: gerador .61, teto 20 CU, E2E via .61).

## Requisitos

- **REQ-904U-001 checkpoint entregável**: um único diretório `laya_t904_unified` carregável por `laya.load()` (pesos, `rl_agent_config.json`, `questions.json`, `encoder/`, `tokenizer/`), recuperado da VM com SHA VM=local, smoke local PASS.
- **REQ-904U-002 estudante único**: inferência executa todas as 12 perguntas `QUESTIONS_V1` num só modelo; sem ensemble T900/T902/T903 em produção.
- **REQ-904U-003 contratos intocados**: `QUESTIONS_V1` rev 1 byte-exato; 19 tópicos + 5 WorkflowPlanning; sem IDL/Rust/validator/mapper alterados.
- **REQ-904U-004 dados limpos**: zero treino/ajuste/seleção/teste com golden450, sealed T902 (`db457693…`), sealed T903 (`7578942a…`), `v4_test.jsonl`; auditoria de n-gramas contra o que é localmente verificável; famílias agrupadas nos splits.
- **REQ-904U-005 rótulos com evidência**: política v3; evidência extraível por linha; dupla anotação em subamostra estratificada com κ reportado; nada rotulado de golden humano.
- **REQ-904U-006 destilação honesta**: soft targets de professores VERIFICADOS por SHA, escolhidos por métrica no dev comum (`teacher_registry` congelado antes do sealed); rótulo auditado prevalece; composição via `gold` + `soft-ce` documentada como soft-CE (não KL-temperatura/RLCD).
- **REQ-904U-007 ablações controladas**: A vs C e B vs D com mesmas linhas/seeds/budget/avaliação; B0/B1/B2 eval-only; E/F NOT_RUN documentado.
- **REQ-904U-008 sealed único**: freeze do finalista (SHA+calibração+thresholds+plano) ANTES da abertura única; gates pré-registrados; sem re-busca após falha.
- **REQ-904U-009 gates numéricos** (no novo sealed T904): QoS Macro-F1 ≥ 0.85; STREAM P ≥ 0.75, R ≥ 0.80, F1 ≥ 0.77; urgency Macro-F1 ≥ 0.90; planning acc ≥ 0.98; complexity Macro-F1 ≥ 0.94; caps médio ≥ 0.97 sem regressão grave individual; sem regressão operacional material vs T900. Status só: `KEEP_T900` | `T904_OFFLINE_ELIGIBLE_AWAITING_APPROVAL` | `BLOCKED_WITH_TRAINED_CANDIDATE`.
- **REQ-904U-010 sistema**: shadow real (`RealLayaAdapter`→DDS→validator) com `tasks_seen=0`/`tasks_written=0`; E2E pareado rules×T900×T904 ou `NOT_RUN`.
- **REQ-904U-011 custo**: teto 20 CU monitorado por fase; parar/relatar ao atingir.
- **REQ-904U-012 entrega**: `laya_t900_handoff/t904/` completo (models/datasets/manifests/reports/logs/scripts) + ZIP com SHA + `decision.json` com valores reais + reprodução via scripts.

## Não objetivos

Treinar T905; promover produção automaticamente; reabrir sealeds históricos; mudar mapper QoS; fusão de pesos como pipeline principal (F só exploratório, aqui NOT_RUN); alegar "reúne o melhor de todos" sem prova pareada.
