# Spec 010 — Spike de interoperabilidade + benchmark

**Fase:** 0b · **Depende de:** 000-dds-contract · **É um GATE:** decide se o ganho justifica.

## Por quê
Antes de migrar qualquer componente de produção, precisamos **provar** que um nó Rust
interopera com o Python e o C++ nos mesmos tópicos, e **medir** o ganho de desempenho. Se o
número não justificar (a análise diz que sim, mas medimos para confirmar), o líder reavalia.

## O quê (requisitos)
- **REQ-101 — Rust→Python.** Um binário Rust publica um `Task`; o orquestrador (ou um stub)
  Python o consome corretamente (campos íntegros). *Aceite:* execução cross-process mostra o
  Task recebido no Python idêntico ao publicado.
- **REQ-102 — Python→Rust.** O Python publica um `Task`/`TaskOutput`; o Rust o consome.
  *Aceite:* o binário Rust imprime/afirma os campos corretos.
- **REQ-103 — Rust↔C++ (LLM).** O Rust publica `LLMInferenceRequest`; o `llama-server` C++
  (com `--enable-dds`) responde `LLMInferenceResult` real; o Rust recebe. *Aceite:* resposta
  de inferência real recebida em Rust. *(Requer o binário C++ e um modelo — pode rodar no host.)*
- **REQ-104 — Benchmark.** Medir latência round-trip (p50/p95/p99) e throughput de um
  `Task`/`TaskOutput` no MESMO host: **Rust-vs-Python** nos mesmos tópicos. *Aceite:*
  `REPORT.md` com **números reais** (não estimados), metodologia e nº de amostras.
- **REQ-105 — Streaming.** Interop de streaming: N chunks de `TaskOutput` com `seq_num`
  0..N sem gaps entre Rust e Python. *Aceite:* teste conta chunks e ordem.

## Fora de escopo
- Implementar o agente/dataspace de produção (fases 1–2). Aqui é **spike**: código
  descartável/mínimo só para provar interop e medir.

## Perguntas abertas
- ~~`[NEEDS-CLARIFICATION]` — o `llama-server` C++ está buildado com DDS neste host?~~
  **Resolvido em 2026-07-17:** NÃO. `src/llama_cpp/build/` contém artefatos macOS arm64
  (Mach-O, `.dylib`) de outra máquina; não há `llama-server` ELF neste host.
  Pré-requisito: build Linux com `cmake -DLLAMA_DDS=ON` (PLANO_EXECUCAO.md, WF-1.4).
  Os demais REQ não dependem dele.
