# Tasks 000 — Contrato DDS

Estado: `[ ]` a fazer · `[~]` em progresso · `[x]` feito · `[!]` bloqueado.
Cada task cita REQ e tem critério de aceite. Ordem = dependência.

- [x] **T-001 · Validar o idlc no IDL real** (REQ-001, riscos)
  Rodar `cargo run -p cyclonedds-idlc -- --input ../../src/llama_cpp/dds/idl/OrchestratorDDS.idl --output-dir /tmp/gen` e inspecionar a saída.
  *Aceite:* todos os tipos do IDL geram sem erro; anotar no REPORT quais construtos existem.
  Se algo não gerar → `[!]` + `[NEEDS-CLARIFICATION]` ao líder.
  **Status:** Implementado via build.rs + cyclonedds-build. IDL compilado com sucesso.

- [x] **T-002 · build.rs gera os tipos** (REQ-001)
  Implementar `build.rs` chamando `cyclonedds-build` sobre o IDL para `OUT_DIR`; `lib.rs`
  inclui sob `#[cfg(feature="dds")]`.
  *Aceite:* `cargo build -p dds-contract --features dds` ✓ e os tipos são acessíveis.
  **Status:** build.rs implementado, compila OrchestratorDDS.idl + OrchestratorV4.idl.

- [x] **T-003 · Constantes e nomes canônicos** (REQ-006, REQ-007)
  `roles.rs` (strengths) + revisar `topics::`/`profiles::` (já no scaffold).
  *Aceite:* teste compara com CONTEXT.md §3; `cargo test -p dds-contract` ✓ (sem dds).
  **Status:** topics::, profiles::, roles::, typenames:: implementados. 3 testes verdes.

- [x] **T-004 · Perfis de QoS online/estrutural** (REQ-004)
  `qos.rs`: `qos_profile(&str) -> (StructuralQos, OnlineKnobs)` para os 5 perfis, valores
  do `profile_mapper.py`.
  *Aceite:* teste cobre os 5; `QoS_StreamLike` lease=1.0; knobs mutáveis corretos.
  **Status:** qos.rs implementado com 5 perfis. all_profiles() retorna todos.

- [x] **T-005 · Teste de wire-compat (typename + @key)** (REQ-002, REQ-003)
  Teste que parseia o IDL (regex `module`/`struct`/`@key`) e confere typename e chaves dos
  6 tipos; afirma LLM* keyless.
  *Aceite:* `cargo test -p dds-contract --features dds` ✓; falha se algum tipo divergir do IDL.
  **Status:** dds_tests::wire_typenames_match_idl_modules, llm_types_are_keyless, v4_keys_match_pragma_keylist.

- [x] **T-006 · Round-trip de serialização** (REQ-005)
  Para cada tipo, construir instância, serializar (XCDR) e desserializar; comparar.
  *Aceite:* teste round-trip por tipo passa com `--features dds`.
  **Status:** dds_tests::roundtrip_llm_request, roundtrip_llm_result, roundtrip_llm_error, roundtrip_task.

- [x] **T-007 · REPORT.md da fase** (Roadmap gate)
  Escrever `specs/000-dds-contract/REPORT.md`: decisão build.rs vs generated commitado,
  construtos do IDL, testes, e handoff.
  *Aceite:* relatório existe; `cargo clippy -p dds-contract -- -D warnings` e `fmt` ✓.
  **Status:** REPORT.md escrito em 2026-07-14. Checkbox sincronizado em 2026-07-17 (WF-0).

## Gate de saída (Fase 0a)
`cargo build -p dds-contract --features dds` ✓ · REQ-001..007 com testes verdes · REPORT escrito.
