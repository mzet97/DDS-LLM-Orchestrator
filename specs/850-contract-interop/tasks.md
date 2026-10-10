# specs/850-contract-interop — Contrato e interop (pré-congelamento)

Fonte: `PLANO_IMPLEMENTACAO_RESTANTE.md` (raiz) Fase 850. Pré-requisito do congelamento da campanha. Mudança **coordenada REQ-003 nos 3 lados** (Rust/Python/C++).

| Task | Escopo | Estado |
|---|---|---|
| T-850-01 | IDL: `ModelSpecialization` + `MS_TRANSCRIPTION`; `TaskPriority` documentado com consts da escala da aplicação (1/5/10); espelhos idênticos; gate t808 ajustado | [ ] |
| T-850-02 | D1: `dds_types.py` `ToolCallRequest` + `requester_id` (ordem CDR do IDL); guarda Python↔IDL estendida aos 14 tipos V4 (campos/ordem/keys/typenames) | [ ] |
| T-850-03 | D2/D3/D4: `Context.Update` TL no Rust; `ToolCall.Request` KL10 no Rust; ponte C++ `LLM.Result` KL8→256 (nos 2 forks); retratação no notes.md | [ ] |
| T-850-04 | T1–T6: orquestrador HTTP preenche `t_serialization/t_transport_send/t_deserialization_ns`; ponte C++ seta `emitted_at_ns`; validação | [ ] |
| T-850-05 | Repro do crash de concorrência (>2 sync) pós-fixes-820; issue/contorno se persistir | [ ] |
| T-850-06 | Gates workspace + REPORT + notes.md (retratações D1..D7) | [ ] |

## Desvios e evidências parciais (PR #9, 2026-10-09)

**Desvio registrado:** as tasks da fase 870 (deploy) foram marcadas `[x]` e
artefatos da fase 880 (campanha) foram adicionados enquanto esta fase segue
pendente — embora 850 seja pré-requisito do congelamento da campanha. Esta
fase passa a ser a **primeira fase ativa**: reconciliar (concluir ou
re-planejar) antes do freeze; 870/880 valem como evidência de deploy/campanha,
não como substituto do contrato.

**Parciais já em código (não remarcar sem completar o escopo):**
- T-850-02 parcial: `requester_id` presente em
  `src/orchestrator/dds_backend/dds_types.py:244` (com marcador `T-850-02`,
  ordem CDR do IDL); a guarda Python↔IDL estendida aos 14 tipos V4 NÃO foi
  verificada.
- T-850-03 parcial: D2 (`Context.Update` TransientLocal) e D3
  (`ToolCall.Request` KeepLast(10)) confirmados em
  `crates/dds-dataspace/src/qos.rs`; D4 (ponte C++ `LLM.Result` KL8→256 nos
  2 forks) NÃO verificado; retratação no notes.md pendente.
