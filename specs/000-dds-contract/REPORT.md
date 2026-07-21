# Report 000 — Contrato DDS (Fase 0a)

**Data:** 2026-07-14 · **Adendo WF-3:** 2026-07-17
**Status:** ✅ Concluído (ampliado em 2026-07-17 — contrato COMPLETO)
**Gate:** `cargo build -p dds-contract --features dds` ✓ · REQ-001..007 com testes verdes

> **Adendo 2026-07-17 (WF-3):** o V4 foi estendido de 4 para **14 tipos** (os 10 que só
> existiam no Python: QoSRoutingProfile, ContextSnapshot, ContextUpdate, ToolCallRequest,
> ExecutionTraceEvent, SecurityPolicySnapshot, SecurityPolicyUpdate, QoSMetric,
> QoSViolation, DiscoveryEvent), corrigindo o drift `dds_types.py`↔IDL (Task +7 campos,
> TaskOutput +2, SystemMetric `double`→`float`). **TypeIds idlc verificados byte-a-byte
> contra os anunciados pelo Python em SEDP nos 14 tipos.** Testes: 20 (lib) + 4
> (`tests/contract_v4.rs`: typenames, keys, blobs de metadata, round-trips XCDR1).
> Codegen passou a emitir `PartialEq` nos structs.

---

## Resumo

A crate `dds-contract` implementa o contrato DDS único para a migração Python→Rust.
Os tipos são gerados automaticamente dos IDLs canônicos (`OrchestratorDDS.idl` + `OrchestratorV4.idl`)
via `cyclonedds-build`, eliminando o drift entre Python/C++/Rust.

## Decisões Técnicas

### build.rs vs Generated Commitado
**Decisão:** build.rs gera os tipos em `OUT_DIR` durante o build.
**Motivo:**
- Reprodutível: sempre regenera do IDL canônico
- Sem artefatos versionados: não há `src/generated/` commitado
- Feature-gated: só compila IDL com `--features dds` (mantém `cargo check` rápido)

### Sanitização de #pragma keylist
O IDL V4 usa `#pragma keylist` (não suportado pelo parser built-in do cyclonedds-build).
O build.rs sanitiza para anotações `@key` antes de compilar.

### Typenames qualificados
O build.rs injeta `#[dds_typename("module::Struct")]` para garantir wire-compat com C++/Python.

## Construtos do IDL

### OrchestratorDDS.idl (module `orchestrator`)
| Tipo | @key | Tópico |
|------|------|--------|
| LLMInferenceRequest | keyless | LLM.InferenceRequest |
| LLMInferenceResult | keyless | LLM.InferenceResult |
| LLMInferenceError | keyless | LLM.InferenceError |
| ServerStatus | keyless | ServerStatus |

### OrchestratorV4.idl (module `dds_llm_orchestrator`)
| Tipo | @key | Tópico |
|------|------|--------|
| Task | task_id | Tasks |
| AgentState | agent_id | AgentRegistry |
| TaskOutput | task_id, seq_num | TaskOutput |
| SystemMetric | metric_name, component_id | SystemMetrics |

## Testes

| Teste | REQ | Status |
|-------|-----|--------|
| topics_match_context | REQ-007 | ✅ |
| profiles_match_nfcm | REQ-007 | ✅ |
| typenames_are_module_qualified | REQ-002 | ✅ |
| wire_typenames_match_idl_modules | REQ-002 | ✅ (feature dds) |
| llm_types_are_keyless | REQ-003 | ✅ (feature dds) |
| v4_keys_match_pragma_keylist | REQ-002 | ✅ (feature dds) |
| idl_file_llm_structs_are_keyless_by_source | REQ-003 | ✅ (feature dds) |
| roundtrip_llm_request | REQ-005 | ✅ (feature dds) |
| roundtrip_llm_result | REQ-005 | ✅ (feature dds) |
| roundtrip_llm_error | REQ-005 | ✅ (feature dds) |
| roundtrip_task | REQ-005 | ✅ (feature dds) |
| roundtrip_agent_state | REQ-005 | ✅ (feature dds) |
| roundtrip_task_output | REQ-005 | ✅ (feature dds) |

**Total:** 10 testes verdes (sem feature dds) + 10 testes verdes (com feature dds) = **20**
*(corrigido em 2026-07-17 — a versão anterior dizia "10 + 7"; o código atual tem 10 gated:
os 4 de wire/keyless + 6 round-trips, incluindo `roundtrip_agent_state` e `roundtrip_task_output`
adicionados após a primeira versão deste relatório)*

## Verificação

```bash
cargo test -p dds-contract                    # 10 passed
cargo clippy -p dds-contract -- -D warnings   # No issues found
cargo fmt -p dds-contract --check             # OK
```

## Handoff para Fase 0b (010-interop-spike)

A crate `dds-contract` está pronta para ser usada pela próxima fase:
- Tipos gerados acessíveis via `dds_contract::generated::orchestrator::*` e `dds_contract::generated::dds_llm_orchestrator::*`
- QoS profiles via `dds_contract::qos_profile("QoS_Critical")`
- Topics/names via `dds_contract::topics::*` e `dds_contract::typenames::*`
- Roles via `dds_contract::roles::*`

A Fase 0b (interop-spike) pode agora:
1. Usar os tipos gerados para criar um nó Rust mínimo
2. Publicar/assinar tópicos DDS reais
3. Interopera com Python+C++ nos mesmos tópicos
4. Benchmark round-trip Rust-vs-Python

## Riscos e Mitigações

| Risco | Status | Mitigação |
|-------|--------|-----------|
| cyclonedds-idlc não gera todos os construtos | ✅ Validado | build.rs + cyclonedds-build funciona |
| #pragma keylist não suportado | ✅ Mitigado | Sanitização para @key no build.rs |
| Typename drift | ✅ Eliminado | Injeção automática de dds_typename |
| Build C do CycloneDDS lento | ✅ Aceito | Feature-gated; só compila com --features dds |
