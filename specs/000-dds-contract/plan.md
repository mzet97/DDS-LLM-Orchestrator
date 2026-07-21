# Plan 000 — Contrato DDS (como)

Implementa a spec `000-dds-contract`. Crate `crates/dds-contract`.

## Abordagem
Gerar os tipos no **build.rs** (reprodutível, sem artefato gerado versionado), usando a
API `cyclonedds-build::compile_idl_with_options` (a mesma que o `cyclonedds-idlc` CLI usa).
Fallback: rodar o CLI e commitar `src/generated/` se o build.rs se mostrar frágil
(decisão registrada no REPORT).

## Estrutura da crate
```
crates/dds-contract/
├── Cargo.toml            # feature `dds` liga cyclonedds + build.rs
├── build.rs              # chama compile_idl sobre OrchestratorDDS.idl -> OUT_DIR/generated.rs
├── src/
│   ├── lib.rs            # topics::, profiles::, include!(generated) sob feature dds
│   ├── qos.rs            # perfis: StructuralQos + OnlineKnobs via QosBuilder
│   └── roles.rs          # STRENGTH_CLIENT/AGENT/ORCHESTRATOR
```

## Detalhes técnicos
1. **build.rs (feature `dds`):** localizar o IDL por caminho relativo do `CARGO_MANIFEST_DIR`
   (`../../../llama_cpp/dds/idl/OrchestratorDDS.idl`); chamar a compilação; escrever em
   `OUT_DIR`. `lib.rs` faz `include!(concat!(env!("OUT_DIR"), "/generated.rs"))` sob
   `#[cfg(feature="dds")]`.
2. **QoS (qos.rs):** para cada perfil, uma função retorna:
   - `StructuralQos` (Reliability/Durability/History/Ownership.kind) — aplicada **na criação**;
   - `OnlineKnobs { transport_priority, latency_budget, ownership_strength }` — aplicáveis em
     runtime. Mapear os valores do Python (`fuzzy_qos_manager/profile_mapper.py`; lembrar
     `QoS_StreamLike.liveliness_lease_s == 1.0`).
3. **Wire-compat:** os tipos gerados já vêm com typename do módulo IDL (`orchestrator::…`) e
   as @key do IDL. O teste REQ-002/003 lê o IDL (parse simples por regex de `@key`/`struct`)
   e compara com metadados do tipo gerado (ou com asserts fixos derivados do IDL).
4. **Sem feature `dds`:** `lib.rs` expõe só `topics`, `profiles`, `roles` (sem os tipos
   gerados) — mantém `cargo check --workspace` rápido.

## Estratégia de teste
- Unit (sempre): `topics`/`profiles`/`roles` batem com CONTEXT.md; qos_profile devolve os 5.
- Integração (`--features dds`): round-trip por tipo; typename/keys conferem; LLM keyless.
- O teste de wire-compat com o C++ real é da Fase 0b (interop-spike); aqui basta o
  typename/keys idênticos ao IDL.

## Orçamento
Não é hot path; foco em corretude. build.rs deve compilar em minutos (build do CycloneDDS
uma vez, cacheado).
