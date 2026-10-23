# traceability — T904 Unified

| Req | Implementação | Verificação | Evidência |
|---|---|---|---|
| 001 checkpoint entregável | vm_phase2_train + fetch + assemble local | verify_checkpoint.py (laya.load + smoke) | manifests/manifest_sha256.json |
| 002 estudante único | 1 treino por braço; eval sem ensemble | vm_t904_eval (1 forward/linha) | reports/dev_comparison.json |
| 003 contratos | import questions.py direto; zero .rs/IDL tocado | contracts_audit.md; cargo test M8 | git status |
| 004 dados limpos | builder proibições + 8-gram + splits por família | summary + split_manifest | datasets/*.summary.json |
| 005 rótulos c/ evidência | dupla anotação 12% + adjudicação | κ em annotation_agreement.json | reports/ |
| 006 destilação honesta | teacher_registry dev-first; gold condicionado ao rótulo | apply_distillation | manifests/teacher_registry.json |
| 007 ablações | A/B/C/D mesmas linhas/seeds | logs por braço | logs/train_*.log |
| 008 sealed único | decision_plan_presealed.json antes da abertura | acesso registrado | reports/sealed_t904_result.json |
| 009 gates | gates pré-registrados em experiment_plan/spec | regression_gates.json | reports/ |
| 010 sistema | shadow harness + E2E .61 | tasks_seen=0; e2e_result | reports/shadow_result.json |
| 011 custo | Budget.check por fase | budget_usage.jsonl | specs/…/budget.md |
| 012 entrega | t904/ completo + ZIP + decision.json | smoke + SHA | FINAL em reports/REPORT_T904.md |
