# tasks — T904 Unified (G/W/T; status honesto)

- **T904U-00 inventário** — GIVEN arquivos históricos, WHEN inspecionar SHAs, THEN inventário completo verificado×reportado×ausente. **DONE** (checkpoint_inventory.json; professores carregam via laya.load local).
- **T904U-01 contrato** — GIVEN questions.py, WHEN extrair, THEN rótulos/IDs/tipos byte-exatos no gerador/builder. **DONE** (contracts_audit.md; import direto do módulo).
- **T904U-02 rótulos** — GIVEN corpus gerado, WHEN dupla anotação + adjudicação, THEN evidência por linha, κ reportado, ambíguos excluídos. **IN_PROGRESS** (gerador na .61).
- **T904U-03 splits** — GIVEN famílias, WHEN dividir, THEN zero overlap + sealed segregado (chmod 000) congelado antes do treino. **TOOL PRONTO** (split_dataset.py; rodar após M1).
- **T904U-04 soft targets** — GIVEN professores verificados, WHEN inferir no dev/distill, THEN gold por qid vencedor sem sobrescrever rótulo auditado. **TOOL PRONTO** (vm_t904_phase1 + apply_distillation).
- **T904U-05 treinos Colab** — GIVEN plano de ablações, WHEN executar, THEN logs/SHAs/checkpoints por braço + CU ≤ 20. **PENDENTE M5**.
- **T904U-06 dev** — GIVEN treinos, WHEN avaliar, THEN métricas por qid/classe + calibração + ICs. **PENDENTE M6**.
- **T904U-07 sealed** — GIVEN finalista congelado, WHEN abrir 1×, THEN protocolo pré-registrado executado. **PENDENTE M7**.
- **T904U-08 shadow** — GIVEN checkpoint experimental, WHEN worker+DDS+Rust, THEN decisões tipadas e tasks_written=0. **PENDENTE M8** (harness run_shadow_t904 parametrizável).
- **T904U-09 E2E** — GIVEN planner LLM (.61), WHEN pareado rules×T900×T904, THEN métricas de workflow; senão NOT_RUN. **PENDENTE M9**.
- **T904U-10 recuperação** — GIVEN pesos na VM, WHEN baixar, THEN laya.load local + hashes iguais. **PENDENTE M10**.
- **T904U-11 decisão** — GIVEN resultados, THEN KEEP_T900 / T904_OFFLINE_ELIGIBLE_AWAITING_APPROVAL / BLOCKED_WITH_TRAINED_CANDIDATE sem trocar produção. **PENDENTE M10**.
