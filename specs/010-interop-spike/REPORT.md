# Report 010 — Spike de interoperabilidade + benchmark (Fase 0b)

**Data:** 2026-07-17 (execução real neste host — amdr7, Ryzen 9 5900X, Fedora)
**Status:** ✅ **GATE PASSOU** — interop provada nas 3 direções + ganho medido de **58×–156×**
**Recomendação:** **seguir** para Fases 1 (`100-agent`) e 2 (`200-dds-dataspace`)

---

## 1. Matriz de interop (todas as execuções com evidência de log)

| Perna | REQ | Resultado | Evidência |
|---|---|---|---|
| Rust→Rust `Task` | T-101 | ✅ 3/3 tasks, campos validados | `pub-task`/`sub-task`, domínio 50 |
| Rust→Rust streaming `TaskOutput` | T-103 | ✅ **0 gaps em 1000 chunks** | `pub-stream`/`sub-stream`, domínio 55 |
| Python→Rust `Task` | T-102 | ✅ 3/3 tasks íntegras | `py_stub_pub`→`sub-task`, domínio 51 |
| Python→Rust streaming | T-103 | ✅ **0 gaps em 1000 chunks** (sem lax, com TypeInfo) | `py_stub_pub_stream`→`sub-stream`, domínio 53 |
| Rust→Python `Task` | T-102 | ✅ 3/3 tasks íntegras | `pub-task`→`py_stub_sub`, domínio 52 |
| Rust→Python streaming | T-103 | ✅ **0 gaps em 1000 chunks** | `pub-stream`→`py_stub_sub_stream`, domínio 54 |
| Rust↔C++ LLM | REQ-103/T-104 | ✅ **inferência real**: `LLMInferenceRequest` → llama-server (Phi-4-mini Q4_K_M) → `LLMInferenceResult(content="Hello", tokens_completion=2, prompt_eval=238 ms)` | `llm-client`↔`llama-server --enable-dds`, domínio 56 |

**Todos os REQs da fase satisfeitos** (101, 102, 103, 104, 105).

## 2. Benchmark RTT (REQ-104) — números reais

Round-trip `Tasks` → echo `TaskOutput`, mesmo host, metodologia idêntica nos dois lados
(warmup 100, **10.000 amostras válidas cada**, p50/p95/p99; domínios isolados 60/61;
`CYCLONEDDS_URI` default multicast). Artefatos: `scripts/benchmark_python_results.json`,
`scripts/benchmark_rust_results.json`.

| Métrica | Python (`benchmark_rtt.py` + `py_echo.py`) | Rust (`rtt-bench`) | Ganho |
|---|---:|---:|---:|
| min | 0,621 ms | 0,132 ms | 4,7× |
| **p50** | **19,068 ms** | **0,327 ms** | **58×** |
| média | 20,633 ms | 0,270 ms | 76× |
| **p95** | **46,096 ms** | **0,345 ms** | **134×** |
| **p99** | **55,464 ms** | **0,355 ms** | **156×** |
| máx | 71,918 ms | 2,741 ms | 26× |
| desvio | 13,621 ms | 0,084 ms | — |

Confirmação independente (criterion, `benches/roundtrip.rs`, 60k iterações):
`task_roundtrip_rust: [262,42 µs, 263,02 µs 263,70 µs]` — coerente com o harness manual.

**Orçamentos do ROADMAP:** propagação < 5 ms p99 → **0,355 ms (14× abaixo)** ✓ ·
RTT serial ≈ 3.700 round-trips/s (Rust) vs ≈ 49/s (Python) — orçamento ≥ 1000 tasks/s ✓.
O piso de ~20 ms do Python bate com o previsto no CONTEXT.md (poll loop de 20 ms no `dds_backend`).

**Nota de comparabilidade:** o echo Rust roda em thread no mesmo processo (2 participantes,
tráfego via stack RTPS/UDP loopback); o echo Python cruza 2 processos (mesma stack UDP).
A diferença de ordem de grandeza (58×+) é estrutural — poll loops e churn por amostra do
Python — não um artefato do arranjo.

## 3. Bugs e desvios encontrados e corrigidos (todos com prova)

1. **`cyclonedds-derive`: `DDS_OP_FLAG_KEY` ausente em campos `#[key]` do tipo `String`.**
   O scan de tamanho de chave do CycloneDDS calculava key=0 bytes → flag `FIXED_KEY` indevida →
   serialização de chave >16 B em buffer estático → **corrupção de heap (`realloc(): invalid pointer`,
   SIGABRT) em todo `dds_write` de tipo keyed com chave >12 B**. Corrigido em
   `cyclonedds-derive/src/lib.rs` (branch `direct_string` honra `is_key` → `adr_key`).
   *Latente também no caminho de leitura (extração de chave por instância).*

2. **Drift IDL↔Python (o que a Fase 0a não viu):** o `dds_types.py` em produção divergia do
   `OrchestratorV4.idl`: `Task` +7 campos (`target_agent` + 6 timestamps `t_*`), `TaskOutput`
   +2 (`agent_id`, `token_count`), `SystemMetric.value` `double`→`float`. IDL alinhado à
   realidade deployada e **TypeIds idlc agora byte-idênticos aos do Python**
   (`Task`: MINIMAL `579d2a90…`/COMPLETE `d4e5ec63…` — verificado contra trace SEDP).
   `OrchestratorV4.{c,h}` regenerados com idlc buildado neste host.

3. **QoS `Ownership=Exclusive` obrigatório nos tópicos v4** (`Tasks`, `TaskOutput`) —
   reader/writer Shared não casam com os endpoints Python (Exclusive). E **ownership strength
   importa**: o stub Python mantém writer ocioso em strength 100; o spike publica com 200.

4. **TypeInformation (XTypes) era obrigatório para peers que validam tipo** (cyclonedds-python,
   llama-server C++): endpoints Rust não anunciavam type info e eram **rejeitados no match**
   (conexão nem se formava). Implementado o pipeline completo: derive ganha
   `#[dds_type_metadata(info,map)]` → `DdsType::type_metadata_blobs()` → `create_topic` anexa
   os blobs (`DDS_TOPIC_XTYPES_METADATA`) → `dds-contract/build.rs` extrai os blobs
   `TYPE_INFO_CDR_*`/`TYPE_MAP_CDR_*` do `.c` do idlc e injeta nos tipos gerados.
   **Desbloqueou Rust→C++ (REQ-103) e Rust→Python.**

5. **Definição de tópico (ktopic) precisa ser idêntica à do peer**, incluindo
   `reliability.max_blocking_time` (10 s) e `liveliness.lease_duration` (10 s em `Tasks`,
   ∞ em `TaskOutput`) — divergência impedia o match mesmo com TypeIds iguais. Perfis do
   spike (`spike_interop::profiles`) agora espelham o SEDP do Python por tópico.

6. **Corrida de discovery (Volatile):** pub que escreve e sai antes do SEDP casar perde tudo.
   Spike usa settle (2,5 s) + `wait_for_acks` — padrão a copiar para os testes de produção.

7. **Stubs do spike corrigidos:** `sys.path` errado (apontava para `src/rust/src/orchestrator`);
   `py_stub_sub_stream` só lê outputs de tasks presentes no tópico `Tasks` → `pub-stream`
   passou a publicar a `Task` dona do stream (semântica de produção). Novos stubs:
   `py_echo.py` (echo do benchmark), `rtt_bench.rs`, `benches/roundtrip.rs`.

8. **Ambiente (pré-requisitos resolvidos):**
   - `CYCLONEDDS_HOME` apontava para prefixo sem `.so` — Python quebrado; prefixo
     `/home/mzet/.local/cyclonedds` reconstruído (estático p/ Rust/C++ + compartilhado p/ Python;
     `ENABLE_SECURITY=OFF`, `ENABLE_SSL=OFF`, `BUILD_IDLC=ON`).
   - `llama-server` Linux x86-64 com `--enable-dds` buildado (`/home/mzet/.cache/llama-build`;
     o `src/llama_cpp/build/` do repo é artefato macOS arm64 — NEEDS-CLARIFICATION da spec
     respondido: **não havia** binário DDS no host; agora há).
   - `CYCLONEDDS_URI` do cluster (`enp4s0`, `AllowMulticast=false`) **não comunica localmente
     nem entre processos Python** — para testes em host único usar a config default (multicast).
     Pendente avaliar a config do cluster para a Fase 1 (multi-host).
   - `venv313` quebrada (sem `bin/`); uso do Python do sistema (3.14 + cyclonedds funcional).
   - `CYCLONEDDS_STATIC` agora dispara rebuild (`rerun-if-env-changed` adicionado ao `-sys`).
   - `CARGO_TARGET_DIR` fora do SMB (`.cargo/config.toml` → `/home/mzet/.cache/tese-rust-target`).

## 4. Verificação

```bash
cargo check --workspace                                        # ✓
cargo test -p qos-nfcm                                         # 7/7 ✓
CYCLONEDDS_STATIC=1 cargo test -p dds-contract --features dds  # 20/20 ✓ (--test-threads=1)
CYCLONEDDS_STATIC=1 cargo build -p spike-interop --features dds # ✓ (7 bins + bench)
CYCLONEDDS_STATIC=1 cargo bench -p spike-interop --features dds # criterion ✓
```

## 5. Handoff e riscos para as próximas fases

- **Copiar os perfis QoS do spike** (`crates/spike-interop/src/lib.rs::profiles`) para a
  `dds-dataspace`/`dds-contract` de produção — incluem ownership/strength, reliability 10 s,
  liveliness por tópico (10 s em Tasks, ∞ em TaskOutput), deadline 10 s em TaskOutput,
  latency 50 ms, tprio 8. Sem isso o agente Rust não casa com a malha Python.
- **Auditar o caminho async da crate** (`async.rs`: `ptr::read` + `dds_return_loan` em tipos
  com ponteiros heap — risco de double-free) **antes** da T-304 (`take_aiter` com strings).
- Padrão settle+`wait_for_acks` nos testes cross-process (corrida de discovery).
- `find_topic(GLOBAL)` **não** funcionou para herdar TypeInfo (timeout mesmo com o peer no ar)
  — não investigar de novo; o caminho é TypeInfo local (blobs idlc), já implementado.
- Artefatos de debug mantidos: `bin/repro_layout.rs` (layout/heap), `bin/dump_ops.rs` (ops do
  descritor). `llama-server` DDS: rebuildar ao mudar o IDL LLM (`OrchestratorDDS.idl`).
- CFT da crate é writer-side (não-SQL) — verificar uso de `ContentFilteredTopic` no
  `dds_backend` ao portar a Fase 2.
- Commits pendentes de revisão do líder (toda a sessão está não-commitada por política).
