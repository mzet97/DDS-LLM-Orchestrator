# Arquitetura T902
prompt/cenario -> curadoria (politica rev.1, evidence_span, REVIEW) -> dataset limpo (grupos) -> splits (train/dev/cal/sealed lacrado) -> treino Colab T4 (laya-train) -> avaliacao dev (API laya.load) -> freeze (model card+SHA) -> sealed 1x -> decision.json -> shadow DDS real (validator Rust, tasks=0) -> handoff.
Nenhum fluxo altera QUESTIONS_V1/IDL/Rust; ABSTAIN existe so na curadoria.
