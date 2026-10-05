# REPORT — specs/850-contract-interop (2026-10-05)

Fonte: `PLANO_IMPLEMENTACAO_RESTANTE.md` Fase 850. **5/5 tasks ✅** — pré-requisitos do congelamento encerrados. Gates: fmt ✅ · clippy `-D warnings` ✅ · workspace **440 passed / 0 failed** · guarda Python↔IDL **53 passed**.

## T-850-01 — IDL alinhado (D7 ✅)
- `ModelSpecialization` + `MS_TRANSCRIPTION` (ordinal 3 — já coerente com o domínio Rust); consts documentais `TASK_PRIORITY_LOW/NORMAL/HIGH = 1/5/10` no IDL com a **convenção de wire** (o `long` carrega 1/5/10, não os ordinais) — o IDL vira fonte da verdade.
- Espelhos 3-way byte-idênticos (`cmp`): `dds-contract/idl` ≡ `src/llama_cpp/dds/v4/idl` ≡ `third_party/llama.cpp_dds/dds/v4/idl`.
- **Regeneração idlc local (V11) KEPT**: diff do `.c` = 1 linha de path; TYPE_INFO/TYPE_MAP e TypeIds byte-idênticos (gate `C_MINIMAL_TYPE_ID` intacto); `.h` = só os 3 `#define` novos. `OrchestratorDDS.{c,h,idl}` intocados.
- `orch-common::TaskPriority` e `models.py::TaskPriority` referenciam as consts.

## T-850-02 — D1 ✅ + guarda integral
- `DDSToolCallRequest.requester_id` na posição CDR exata (IDL: entre `request_id` e `tool_name`); conversores `tool_call_to_dds`/`dds_to_tool_call` mapeiam; default `""` mantém compat (construções são keyword-only).
- `test_idl_python_consistency.py` reescrito **data-driven para os 17 tipos** (14 V4 + 3 LLM): typename (módulo extraído do IDL), campos em ordem, chaves; meta-gate (IdlStruct novo sem entrada falha) + unicidade de typenames. **Achado: exatamente 1 drift real — o próprio D1.** Zero outro desvio em campos/ordem/keys nos 17 tipos.

## T-850-03 — D2/D3/D4 ✅ (unificadas, Python canônico)
- **D2**: Rust `context_update()` Volatile → **TransientLocal** (RxO: writer TL serve ambos; late-joiner do context-store recebe a última atualização).
- **D3**: Rust `tool_call` KL5 → **KL10** (paridade Python).
- **D4**: ponte C++ `LLM.InferenceResult` KL8 → **KL256 + limits + DurabilityService** — o perfil profundo (Gate C2) já existia no fork canônico; **portado para a cópia de trabalho** `src/llama_cpp` (writer + reader do modo cliente). Compilação C++ real verificada: `cmake --build --target llama-server` **100% nas duas árvores**.

## T-850-04 — T1–T6 ponta a ponta ✅
- **Orquestrador HTTP**: T1 (serialização, no wire), T2 (transport_send — medido, reportado na resposta HTTP pois só existe pós-write; wire honesto 0), T5 (`wall − completed_at_ns` do terminal, saturado), T6 (duração da leitura terminal); T3/T4 seguem do agente; FAILED carrega as 6 medidas.
- **C++ `emitted_at_ns`**: carimbado no choke point `DDSTransportImpl::send_response` com `dds_time()` (cobre parciais, finais e erros) — nos 2 forks.
- **Validação ao vivo (mock engine, /sync):** resposta carregou os 6 campos: T1≈26 µs, T2≈48 µs, T3≈250 ms, T4≈255 ms, T5≈14 ms, T6≈5 µs.

## T-850-05 — Crash de concorrência: **NÃO REPRODUZÍVEL** ✅
Repro local (DDS loopback, orquestrador static + 1 agente mock slots 8): **960 requisições /sync em 3 rodadas — c=8×30, c=4×60, c=16×30 — 100% HTTP 200, zero 504, orquestrador vivo, 23 fds**. O crash de julho (">2 concorrentes") não se manifesta no código pós-850 — os P0 corrigidos em 820 (admission, caches, ack, republisher) são a explicação mais provável. **Encerrado como resolvido/não-reproduzível**, com o cenário registrado; vigiar na campanha (EXP3 c=8 real).

## Pendências conscientes
- `policy_engine`/`mcp_gateway` ainda derivam `request_id` do `agent_id` (identidade é M2, política permissiva) — `requester_id` agora existe e trafega; uso efetivo é decisão de M2.
- D5 (C++ sem writer de `LLM.InferenceError`): resolvido no **fork canônico** (`send_error`/BRIDGE-STATE-013); a cópia de trabalho `src/llama_cpp` não o tem (sincronização de árvore pendente — 870 redeploy usa o canônico).
- Testes D2/D3 são `cfg(dds)` — executam no gate full (feature unification do workspace).
