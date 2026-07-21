# Tasks 010 — Spike de interop + benchmark

> **Histórico:** as 6 tasks estavam marcadas `[x]` em 2026-07-16 sem aceite executado
> (corrigido em WF-0). Em **2026-07-17** todas foram executadas de verdade — evidências
> e números no `REPORT.md` (gate **PASSOU**).

- [x] **T-101 · Crate spike + pub/sub Rust** (REQ-101/102)
  Criar `crates/spike-interop`; `pub_task.rs` publica Tasks, `sub_task.rs` assina.
  *Aceite:* dois binários Rust trocam um Task no mesmo domínio (afirma campos).
  **Status:** ✅ Executado 2026-07-17 — 3/3 Tasks recebidas com campos validados (domínio 50).
  Exigiu correção do bug de `DDS_OP_FLAG_KEY` no derive (crash de heap).

- [x] **T-102 · Stubs Python** (REQ-101/102)
  `scripts/py_stub_pub.py`/`py_stub_sub.py` usando o `dds_backend` Python.
  *Aceite:* Rust→Python e Python→Rust: Task recebido íntegro (documentar no REPORT).
  **Status:** ✅ Executado 2026-07-17 — ambas as direções com campos íntegros (domínios 51/52).
  Exigiu: alinhamento do IDL (drift Task/TaskOutput/SystemMetric), QoS Exclusive,
  TypeInformation nos endpoints Rust e ktopic QoS idêntico ao do peer.

- [x] **T-103 · Interop de streaming** (REQ-105)
  Publicar N `TaskOutput` com seq_num; contar gaps do outro lado.
  *Aceite:* 0 gaps em ≥ 1000 chunks, Rust↔Python.
  **Status:** ✅ Executado 2026-07-17 — **0 gaps em 1000 chunks nas 3 direções**:
  Rust→Rust (55), Python→Rust (53), Rust→Python (54).

- [x] **T-104 · Interop LLM Rust↔C++** (REQ-103)
  `llm_client.rs` publica `LLMInferenceRequest`; recebe `LLMInferenceResult` do llama-server.
  *Aceite:* resposta de inferência real recebida em Rust.
  **Status:** ✅ Executado 2026-07-17 — llama-server Linux (build novo, `LLAMA_DDS=ON`)
  + Phi-4-mini Q4_K_M; resposta real: `content="Hello"`, 2 tokens, prompt eval 238 ms.

- [x] **T-105 · Benchmark RTT Rust-vs-Python** (REQ-104)
  `benches/roundtrip.rs` (criterion) + medição equivalente no Python; mesma carga/QoS.
  *Aceite:* p50/p95/p99 e throughput dos dois; metodologia documentada; ≥ 10k amostras.
  **Status:** ✅ Executado 2026-07-17 — 10.000 amostras/lado: Rust p50 0,327 / p95 0,345 /
  p99 0,355 ms vs Python p50 19,068 / p95 46,096 / p99 55,464 ms (**58×–156×**).
  Criterion: 262–264 µs (60k iters). JSONs + metodologia no REPORT §2.

- [x] **T-106 · REPORT.md + recomendação de gate** (Roadmap gate)
  Tabela Rust-vs-Python com números reais; recomendação (seguir / reavaliar).
  *Aceite:* REPORT existe; líder revisa antes da Fase 1.
  **Status:** ✅ REPORT.md reescrito com números reais, matriz completa e 8 achados;
  recomendação: **SEGUIR** para Fases 1–2.

## Gate de saída (Fase 0b)
Interop provada (REQ-101/102/103/105) ✓ · benchmark com números reais (REQ-104) ✓ ·
REPORT + decisão do líder: **PASSOU — 58×–156× de ganho, orçamento <5 ms p99 cumprido (0,355 ms)**.
