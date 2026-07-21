# Plan 010 — Spike de interop + benchmark (como)

## Abordagem
Criar uma crate descartável `crates/spike-interop` (bin + exemplos) que usa `dds-contract`
(`--features dds`) e `cyclonedds`. Os testes de interop são **cross-process**: um lado Rust,
outro Python (o orquestrador atual ou um stub mínimo), no **mesmo domínio DDS**.

## Componentes
```
crates/spike-interop/
├── src/bin/pub_task.rs     # publica N Tasks e sai
├── src/bin/sub_task.rs     # assina Tasks e imprime/afirma
├── src/bin/llm_client.rs   # publica LLMInferenceRequest, espera Result (REQ-103)
└── benches/roundtrip.rs    # criterion: RTT Rust-vs-Python
scripts/
├── py_stub_pub.py          # publica Task/TaskOutput (usa o dds_backend Python)
└── py_stub_sub.py          # assina e ecoa (para medir RTT com o Rust)
```

## Detalhes
1. **Mesmo domínio:** fixar `CYCLONEDDS_DOMAIN` e `CYCLONEDDS_URI` iguais nos dois lados.
   Domínio ≤ 232. Rodar Rust e Python em processos separados no mesmo host.
2. **REQ-101/102:** `pub_task` (Rust) ↔ `py_stub_sub.py` (Python) e vice-versa. Comparar os
   campos (task_id, status, payload) — devem bater byte-a-byte na semântica.
3. **REQ-103:** `llm_client.rs` publica `LLMInferenceRequest` (keyless, typename
   `orchestrator::…`); subir `llama-server --enable-dds --dds-domain N`; receber `Result`.
4. **REQ-104 (benchmark):** RTT = publicar Task → o eco volta como TaskOutput; medir com
   `criterion` no lado Rust e um cronômetro equivalente no lado Python (mesmo payload, mesma
   carga, mesmas QoS). Reaproveitar a metodologia de `cyclonedds-bench` (latency/throughput).
   Reportar p50/p95/p99, throughput, e o delta Rust-vs-Python. **Documentar tudo** (nº de
   amostras, warmup, QoS, tamanho do payload) para reprodutibilidade.
5. **REQ-105:** publicar N TaskOutput com seq_num crescente; o outro lado conta gaps.

## Metodologia de medição (honesta)
- Warmup (descartar as primeiras K amostras). ≥ 10k amostras por cenário. Reportar
  intervalo/percentis, não só média. Fixar frequência de CPU se possível (governor
  performance). Registrar `git_commit` e config. **Sem número inventado** (Constituição III).

## Saída
`specs/010-interop-spike/REPORT.md` com a tabela Rust-vs-Python e a recomendação de gate.
