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
