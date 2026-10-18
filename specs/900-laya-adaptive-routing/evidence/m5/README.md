# Evidências M5 — engine/journal/DAG/recovery (T-900-M5-01..04)

Data: 2026-10-17. workflow-engine: **60/0**, clippy -D warnings OK, fmt OK.

| Tarefa | Módulo | Aceite |
|---|---|---|
| M5-01 journal | `journal.rs` (7 tests) | SQLite+WAL+migrations; lock de instância (recicla lock órfão de PID morto); decisão efetiva única (Replay/Conflict tipados); reserva TRANSACIONAL com constraint + chave idempotente (replay não duplica); consumo incerto cobra reserva INTEGRAL (medido devolve excesso); outbox atômico com mudança de estado; IN_DOUBT persiste restart; corrupção → erro tipado (fail-closed) |
| M5-02 DAG | `dag.rs` (9 tests) | ciclo/aresta inexistente/id duplo/capability fora do catálogo/prioridade 1-5-10/attempts 1..3/output ≤1MiB/TOOL rejeitado até V10/prazo/**orçamento agregado** (planner + Σ max_tokens×attempts ≤ teto, sem reduzir silencioso); `DirectTemplatePlanner` (tarefa simples NÃO chama planner); `Planner` trait é a seam p/ LLM real |
| M5-03 scheduler | `engine.rs` (5 tests) | readiness por DEPENDÊNCIA (join não dispara por ordem de chegada); materialização com `laya1-` id físico por tentativa + outbox; DONE valida outputs declarados; idempotência de materialize |
| M5-04 recovery | `engine.rs` (4 tests) | restart retoma a MESMA tentativa (mesmo task_id); done set reconstruído destrava dependentes; saída tardia de tentativa antiga → `StaleAttempt` (não conclui nova); retry cria task_id NOVO (falha cobra integral; FAILOVER habilita retry) |

## Não verificado (gates futuros)

- Publicação DDS de ponta a ponta pelo outbox em processos reais (drain é
  testado com callback; integração ao `DdsWorkflowSpace` no runtime do
  engine bin — M6/canary).
- Planner LLM real via inferência DDS (D7): trait pronta, implementação é
  integração com `LLM.InferenceRequest` — sem campanha (M6 BLOCKED).
- V10 (side-effect/IN_DOUBT com ferramenta real): TOOL rejeitado pelo
  validator até o gate; in_doubt testado no journal.
