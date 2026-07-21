# Spec 000 — Contrato DDS (tipos + perfis de QoS)

**Fase:** 0a · **Crate:** `dds-contract` · **Depende de:** nada · **Desbloqueia:** tudo.

## Por quê (motivação)
Todo nó (Python, C++, Rust) precisa falar o **mesmo wire format**. O Python mantém tipos
DDS **à mão** (`dds_types.py`), o que já causou *drift* com o C++ (os 3 tipos LLM
divergiram e quebraram o XTypes). Em Rust, geramos os tipos **do mesmo IDL** que o C++ usa,
eliminando o drift por construção. Esta crate é a **fonte única de tipos** da migração.

## O quê (requisitos)
Cada requisito tem um ID e um critério de aceite **verificável**.

- **REQ-001 — Geração a partir do IDL canônico.** Os tipos Rust são gerados de
  `src/llama_cpp/dds/idl/OrchestratorDDS.idl` via `cyclonedds-idlc` (não escritos à mão).
  *Aceite:* existe um passo reprodutível (build.rs ou script) que regenera; o arquivo
  gerado não é editado à mão.
- **REQ-002 — Wire-compat com o C++.** Cada tipo tem o **mesmo typename** (`orchestrator::…`)
  e as **mesmas @key** do IDL. *Aceite:* teste que confere typename e campos-chave de
  `Task`, `AgentState`, `TaskOutput`, `LLMInferenceRequest/Result/Error` contra o IDL.
- **REQ-003 — Tipos LLM keyless.** Os 3 tipos `LLM*` são **keyless** (casar a reconciliação
  já feita no Python). *Aceite:* teste afirma ausência de @key nesses 3 tipos.
- **REQ-004 — Perfis de QoS com divisão online/estrutural.** Os 5 perfis
  (`QoS_Critical/Failover/StreamLike/LowCost/Balanced`) expostos como builders, deixando
  explícito o que é **mutável em runtime** (TransportPriority, LatencyBudget,
  OwnershipStrength) vs **estrutural** (Reliability/Durability/History; Deadline unsupported).
  *Aceite:* API `qos_profile(name) -> (StructuralQos, OnlineKnobs)`; teste cobre os 5.
- **REQ-005 — Round-trip de serialização.** Cada tipo serializa e desserializa (XCDR)
  idempotente. *Aceite:* teste round-trip por tipo (com `--features dds`).
- **REQ-006 — Constantes de ownership por papel.** `STRENGTH_CLIENT=10`, `AGENT=100`,
  `ORCHESTRATOR=200` (Fase 2.2). *Aceite:* constantes públicas + teste trivial.
- **REQ-007 — Nomes canônicos.** Tópicos e perfis com strings idênticas ao Python/IDL.
  *Aceite:* `topics::*` e `profiles::ALL` batem com CONTEXT.md §3.

## Fora de escopo
- Lógica de leitura/escrita (é da `dds-dataspace`, Fase 2).
- Qualquer alteração no IDL. Se o IDL precisar mudar, **`[NEEDS-CLARIFICATION]`** ao líder.

## Riscos / perguntas abertas
- `[NEEDS-CLARIFICATION]` — o `cyclonedds-idlc` gera todos os construtos usados no IDL
  (unions, enums, sequences, strings bounded)? Validar na T-001; se algum tipo não gerar,
  reportar antes de contornar à mão.
