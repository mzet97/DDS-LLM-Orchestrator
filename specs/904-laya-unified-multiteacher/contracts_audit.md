# contracts_audit — T904 Unified (M0, 2026-10-21)

Fonte única extraída do CÓDIGO (não de prompts): `rust/workers/laya-worker/src/laya_worker/questions.py`, `QUESTIONS_REVISION = 1`.

## QUESTIONS_V1 (12 qids efetivos)

| qid | tipo | opções/rótulos |
|---|---|---|
| `qos_profile` | choice | QI_BALANCED, QI_CRITICAL, QI_STREAM_LIKE, QI_LOW_COST, QI_FAILOVER (ordem = parte da revisão) |
| `needs_planning` | noul | P(true) |
| `complexity` | score | simple(0), moderate(1), complex(2), very_complex(3) |
| `urgency` | score | low(0), normal(1), high(2) |
| `cap__rust_refactor` | noul | P(true) |
| `cap__python_refactor` | noul | P(true) |
| `cap__unit_testing` | noul | P(true) |
| `cap__security_audit` | noul | P(true) |
| `cap__code_review` | noul | P(true) |
| `cap__research` | noul | P(true) |
| `cap__summarization` | noul | P(true) |
| `cap__translation` | noul | P(true) |

## Verificações de contrato nesta campanha

- O texto de `instructions`/`criteria` para datasets é importado DIRETO de `questions.py` (função `_laya_questions()` do adapter e `QUESTIONS_V1` bruta) — nunca reescrito no gerador.
- Formato SDK 0.4.10 confirmado no código-fonte do SDK (`laya/train.py::items_from_rows`): linha aceita `{state, questions, expected}` (hard) e/ou `gold` (distribuições: choice→{label: p}, noul→{"true"/"false": p}, score→{"0".."k-1": p}); `gold[qid]` prevalece sobre `expected[qid]`; qids sem gold/expected são simplesmente ignorados no treino — **rótulos parciais por linha são suportados nativamente**.
- Losses disponíveis: `soft-ce` e `rlcd` apenas. Destilação = `soft-ce` com `gold` (documento no experiment_plan; NÃO chamamos isso de RLCD/KL-temperatura).
- Worker/publicação: `Routing.Decision` somente; validator Rust `decision.rs`; fronteira shadow (calibração vazia) inalterada.
- 19 tópicos canônicos + 5 WorkflowPlanning: nenhum será alterado (nenhum `.rs`/IDL nesta campanha de treino).
- `evals` CLI 0.4.10 não aceita path local — usar `laya.load(path)` + `laya.evals.evaluate` (já validado nas fases anteriores).

## Veredito

Contratos auditados e CONGELADOS para esta campanha. Qualquer divergência entre gerador e `questions.py` é defeito do gerador (corrigir o gerador, nunca o contrato).
