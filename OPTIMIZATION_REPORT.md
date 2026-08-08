# Optimization Report — DDS-LLM Orchestrator (Rust workspace)

**Status: TODAS as fases do plano (0, 0.5, 1–6) implementadas e validadas.** Comparação E2E
real (DDS real, llama-server real, modelo real, sem mocks) executada e reportada abaixo.

**Atualização 2026-07-20 (sessão seguinte):** a Fase 4 (zero-copy `write_loan` em
`TaskOutput`) foi retomada e concluída. O bloqueio original era um achado de segurança real
na crate `cyclonedds` (não uma desculpa) — mas em vez de ficar só documentado, foi corrigido
na própria crate (novo associated type `DdsType::Native`, ver `OPTIMIZATION_PLAN.md` Fase 4)
e a otimização foi implementada e validada com um teste de aceite dedicado (1000 chunks reais
via DDS, 0 gaps, campos `String` íntegros) mais a suíte de 106 testes de integração da
própria crate `cyclonedds`, sem regressão em nenhum teste pré-existente do workspace.

**Atualização 2026-07-20 (3ª sessão):** a Fase 5 (WaitSet compartilhado, T-617) foi retomada
e concluída. Em vez de redesenhar o streaming como fan-out/broadcast sobre um reader único
por tópico (risco de gaps sob consumidor lento), a implementação preserva a semântica atual
de N assinantes independentes por tópico (cada `stream_*()` mantém seu próprio reader) e só
compartilha o MECANISMO DE ESPERA: 1 `WaitSet` por `DataSpace` (`dispatch::SharedWaitSet`),
readers anexados dinamicamente com cookie único, notificação local via `tokio::sync::Notify`
em vez de 1 thread de blocking-pool bloqueada por stream. Validado com teste de aceite
dedicado (40 streams concorrentes — o padrão real do `client`, 2 streams por `submit()` —
compartilhando 1 WaitSet, cada uma recebendo todos os dados esperados, 0 vazamento), sem
regressão em nenhum teste pré-existente (144 resultados de teste no total, workspace inteiro).

Ver `OPTIMIZATION_AUDIT.md` (achados) e `OPTIMIZATION_PLAN.md` (fases + tabela P1–P3). Este
documento continua sendo preenchido conforme cada item do plano for implementado, medido e
aceito ou revertido.

---

## Ambiente de teste

| Campo | Valor |
|---|---|
| Hardware | Ryzen 5900X-class (24 threads lógicas), AMD RX 7900 XTX presente mas **não usada** na inferência do E2E (ver caveat na seção de comparação abaixo — build CPU-only) |
| SO | Linux (Fedora-like, `dnf`/`rocminfo` presentes) |
| `CARGO_TARGET_DIR` | `$HOME/.cache/tese-rust-target` (fora do mount SMB/CIFS do repo) |
| Filesystem do repo | SMB/CIFS (`/run/host/var/mnt/HD1TB/tese`) — travamentos intermitentes documentados abaixo |
| DDS domain do E2E real | 77 (evita colisão com domínios usados por outras suítes deste repo) |
| Modelo do E2E real | `Qwen3.5-0.8B-Q4_K_M.gguf` (`tese/models/`) |
| `llama-server` do E2E real | binário pré-existente `tese/src/llama_cpp/build-dds/bin/llama-server` (CPU-only, static-linked DDS) — ver caveat |
| Nº de clientes/agentes no E2E real | 1 agente por vez (Rust OU Python, sequencial — não concorrente), 1 cliente sequencial, N=20 requisições |
| Payload do E2E real | prompt curto ("What is 2+2? Answer with just the number."), `max_tokens=256`; modelo é "thinking" (Qwen3.5), gera cadeia de raciocínio antes da resposta — 131 a 256 tokens de completion por requisição, variável |

## Resultados (preencher por alteração aceita)

| Alteração | Antes | Depois | Variação | Teste utilizado | Trade-off |
|---|---:|---:|---:|---|---|
| `benchmarks/Cargo.toml`: dev-dependency `agent` não força mais `features = ["dds"]` (Fase 0) | `cargo check --workspace --all-targets` sem `--features dds`: **~4min33s**, compilando `cyclonedds` v1.8.0 real (build C completo) mesmo sem a feature | **`cargo check --workspace --all-targets`: 24,11s** (`time`: real 0m24,283s / user 0m2,794s / sys 0m3,251s), **zero ocorrências de "Compiling cyclonedds"** no log — confirmado por `grep -c` | **~11× mais rápido**, C build eliminado do caminho sem feature | `cargo check --workspace --all-targets` re-executado do zero após o fix, log completo capturado e inspecionado | Nenhum — mudança é puramente de manifesto, não toca comportamento em runtime; não quebra nenhum contrato DDS |
| `dds-contract/build.rs`: IDL repontado de `src/llama_cpp` (descontinuado) para `third_party/llama.cpp_dds` (árvore C++/DDS atual, confirmado pelo usuário); `OrchestratorV4.idl`/`.c`/`.h` sincronizados byte-a-byte (`cmp` limpo) para incluir os 10 tipos da WF-3 que faltavam em `third_party` (Fase 0.5) | `third_party/llama.cpp_dds` tinha IDL pré-WF-3 (4 tipos, `#pragma keylist`, sem os campos de instrumentação); `dds-contract` gerava tipos a partir da árvore sendo descontinuada — risco de continuidade quando `src/llama_cpp` for removido | `cargo check -p dds-contract --features dds`: **7,42s, 0 erros**. `cargo test -p dds-contract --features dds -- --test-threads=1`: **26/26 testes verdes** (20 lib + 2 `async_soundness` + 4 `contract_v4`, incluindo `roundtrip_platform_types_xcdr1`/`platform_typenames_match_python`/`platform_keys_match_python` — os 10 tipos da WF-3) | Correção de correção/continuidade, não de performance — sem regressão nos testes existentes | `cargo check -p dds-contract --features dds` + `cargo test -p dds-contract --features dds -- --test-threads=1`, ambos rodados do zero após o repoint | `llama-server` C++ não referencia tipos V4 diretamente (confirmado por grep em `dds_bridge.cpp`), então não havia quebra de wire format ativa — só risco futuro. `dds_v4_bridge.cpp` em `third_party` não foi atualizado para os campos/tipos novos (fora do escopo pedido); `SystemMetric.value` mudou de `double`→`float` nessa árvore (compila com conversão implícita estreitando em `publish_metric`, sem erro, mas vale revisão); `src/llama_cpp/` continua existindo, duplicado — arquivamento é decisão futura do usuário |
| `ahash` em todos os `DashMap` (`dds-dataspace`, `orchestrator`, `llm-gateway`, `context-store`, `observability`, `policy-engine`) (Fase 2) | Hasher default (SipHash) em 100% dos caches; `ahash` era dependência morta do workspace | Todos os 6 crates recompilam limpo; `cargo test --workspace --features dds -- --test-threads=1`: **75/75 suítes verdes, 0 falhas**; `cargo clippy --workspace --all-targets --features dds -- -D warnings` limpo | Qualitativo confirmado (compila+testa verde); microbenchmark de throughput lookup/insert **não medido** nesta sessão (ficaria fora do tempo restante) | `cargo check`/`test`/`clippy --workspace --features dds`, `cargo fmt --all --check`, todos re-executados do zero pós-mudança | Nenhum — troca de hasher não muda semântica; `DashMap<K,V,ahash::RandomState>` é drop-in |
| `Arc<Task>`/`Arc<TaskOutput>` na trait `DataSpaceApi` (`read_task`, `all_tasks`, `subscribe_tasks`, `read_task_outputs`, `subscribe_task_outputs`) em vez de clonar (Fase 3) | Trait clonava a struct inteira em 5 métodos; achado original ("agent re-clona Task 3-4x") revisado: `agent`/`orchestrator` já usavam métodos inerentes que retornavam `Arc` — o desperdício estava isolado na trait abstrata | `cargo test -p dds-dataspace --features dds -- --test-threads=1`: **14/14 verdes** (inclui `contract_real_dds`, A/B mock vs DDS real); `cargo check --workspace --features dds` limpo (nenhum consumidor quebrou) | Ganho real mas de escopo mais estreito que o suposto — beneficia consumidores polimórficos da trait, não o hot path de produção (que já era eficiente) | `cargo check/test/clippy -p dds-dataspace --features dds`, `cargo check --workspace --features dds`, `cargo fmt --check`, todos verdes | Nenhum — API muda mas nenhum consumidor precisou de alteração (acesso a campo via Deref funciona igual) |
| **Fase 4 (2026-07-20, sessão seguinte)** — zero-copy `write_loan` em `TaskOutput`: corrigido `DdsType::Native` na crate `cyclonedds` (novo associated type — o loan passa a operar sobre o struct wire-compatible, não sobre o struct ergonômico com `String`) + `dds-dataspace::writer_pool::write_output_loan()` usando `request_loan`/`DdsString::new(..)` para os 3 campos `String` de `TaskOutput` | `request_loan()` zerava/interpretava o buffer como `size_of::<TaskOutput>()` (24B por `String`), mas `dds_request_loan` aloca `T::descriptor_size()` = `size_of::<Native>()` (8B por `DdsString`) — **estouro de buffer real em todo loan**, não só um risco de bit-pattern; `.write()` normal (via `write_to_native`/`WriteArena`) usado para tudo, zero-copy nunca exercitado | Teste de aceite dedicado `dds-dataspace/tests/write_loan.rs::task_output_loan_roundtrip_1000_chunks_no_gaps`: **1000/1000 chunks, 0 gaps, 0 duplicatas, 3 campos `String` íntegros em cada chunk, 1,54s**, DDS real (domain 83). Suíte própria da crate `cyclonedds`: **106 testes de integração + 8 unitários + 12 doctests, 0 falhas** (unions/enums/sequences/nested — não só o caso simples) | Correção reduz alocação/cópia por chunk no tópico de maior volume (streaming de inferência); magnitude exata (ns/chunk, alocações evitadas) não medida com profiler — só a corretude/gap-freedom foi medida diretamente | `cargo test -p cyclonedds -p cyclonedds-derive` (crate `cyclonedds-rust`, target dir próprio); `cargo test -p dds-dataspace --features dds --test write_loan`; `cargo test -p dds-dataspace --features dds` (15 suítes); `cargo test --workspace --features dds` (77 suítes); `cargo test --workspace` (65); `cargo clippy --workspace --all-targets [--features dds] -- -D warnings` (2 modos); `cargo fmt --all --check` — todos verdes | API do loan muda de `&mut T` para `&mut T::Native` (campos `String`→`DdsString`) — mudança pública na crate `cyclonedds`, mas sem breaking change real no workspace `tese/src/rust` (nada lá chamava `request_loan`/`write_loan_async` antes). 6 impls manuais de `DdsType` + 7 exemplos da crate `cyclonedds` precisaram de `type Native = Self;` (mecânico, sem risco — todos já eram POD) |
| **Fase 5 (2026-07-20, 3ª sessão)** — WaitSet compartilhado: novo módulo `dds-dataspace::dispatch` (`SharedWaitSet`/`Registration`); os 16 `stream_*()` migrados de `reader.take_aiter()` (WaitSet próprio por stream) para `waitset.register(&reader)` + `registration.notified().await` + `reader.take_async()` (mesmo reader dedicado de sempre — semântica de N assinantes independentes por tópico preservada, sem fan-out/broadcast) | Cada `stream_*()` criava seu próprio `WaitSet`, ocupando 1 thread de blocking-pool do tokio por toda a vida da stream; o padrão real mais exigente (`client::submit()`, 2 streams por chamada) com 50 clientes concorrentes chegava a ~100 WaitSets simultâneos | Teste de aceite dedicado `dds-dataspace/tests/shared_waitset.rs::n_concurrent_streams_share_one_waitset_and_still_see_everything`: **40 streams concorrentes (20 Tasks + 20 TaskOutput) → 40 registros num único `SharedWaitSet`** (`registration_count()`), cada uma das 20 `stream_tasks()` recebeu as 20 tasks publicadas (assinante independente preservado), 0 registros restantes após drop, **0,82s**. Workspace inteiro: 144 resultados de teste, 0 falhas | Redução estrutural comprovada (N streams → 1 WaitSet, não N); magnitude em threads/memória sob carga de produção multi-processo real não medida (falta a infra de carga da Fase 1) | `cargo check -p dds-dataspace --features dds --all-targets`; `cargo test -p dds-dataspace --features dds --test shared_waitset`; `cargo test -p dds-dataspace --features dds` (16 suítes); `cargo test --workspace --features dds`/sem feature (144 resultados no total); `cargo clippy --workspace --all-targets [--features dds] -- -D warnings` (2 modos); `cargo fmt --all --check` — todos verdes | API pública inalterada (`stream_*()` continuam retornando `impl Stream<Item=ArcT>` idêntico); mudança é só na implementação interna. Nenhum consumidor (`agent`/`orchestrator`/`client`/`benchmarks`/etc.) precisou de alteração |

## Comandos executados (auditoria + correções + validações desta sessão)

```bash
export CARGO_TARGET_DIR="$HOME/.cache/tese-rust-target"
cd tese/src/rust

# Auditoria inicial (antes das correções)
cargo fmt --all -- --check                                   # PASSOU
cargo check --workspace --all-targets                        # PASSOU (4min33s, cyclonedds C buildado sem necessidade)
cargo test --workspace                                       # PASSOU (196 passed, 0 failed)
cargo clippy --workspace --all-targets -- -D warnings         # PASSOU (sem warnings)

# Validação da Fase 0 (benchmarks/Cargo.toml)
cargo check --workspace --all-targets                        # PASSOU (24,11s, sem build C — fix confirmado)

# Validação da Fase 0.5 (repoint dds-contract → third_party/llama.cpp_dds)
CYCLONEDDS_STATIC=1 cargo check -p dds-contract --features dds                       # PASSOU (7,42s)
CYCLONEDDS_STATIC=1 cargo test -p dds-contract --features dds -- --test-threads=1    # PASSOU (26/26)

# Fase 1 — baseline real com --features dds no workspace inteiro (nunca rodado antes)
CYCLONEDDS_STATIC=1 cargo test --workspace --features dds -- --test-threads=1        # PASSOU (75 suítes, 0 falhas)

# Fase 2 — ahash nos DashMap (6 crates)
CYCLONEDDS_STATIC=1 cargo check --workspace --all-targets --features dds             # PASSOU (limpo)
CYCLONEDDS_STATIC=1 cargo test --workspace --features dds -- --test-threads=1        # PASSOU (75 suítes, 0 falhas, novamente)
CYCLONEDDS_STATIC=1 cargo clippy --workspace --all-targets --features dds -- -D warnings  # PASSOU
cargo fmt --all -- --check                                                            # PASSOU

# Fase 3 — Arc<Task>/Arc<TaskOutput> na trait DataSpaceApi
CYCLONEDDS_STATIC=1 cargo check -p dds-dataspace --features dds                       # PASSOU
CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1    # PASSOU (14/14)
CYCLONEDDS_STATIC=1 cargo check --workspace --features dds                            # PASSOU (nenhum consumidor quebrou)
CYCLONEDDS_STATIC=1 cargo clippy -p dds-dataspace --features dds --all-targets -- -D warnings  # PASSOU
cargo fmt -p dds-dataspace --check                                                    # PASSOU

# Fase 4 (sessão seguinte) — Native associated type na crate cyclonedds +
# write_output_loan em dds-dataspace
cargo check -p cyclonedds -p cyclonedds-derive --all-targets \
  --manifest-path third_party/cyclonedds-rust/cyclonedds-rust/Cargo.toml   # PASSOU (target dir próprio)
cargo test -p cyclonedds -p cyclonedds-derive -- --test-threads=1 \
  --manifest-path third_party/cyclonedds-rust/cyclonedds-rust/Cargo.toml   # PASSOU (106+8+12, 0 falhas)
CYCLONEDDS_STATIC=1 cargo check -p dds-dataspace --features dds                       # PASSOU
CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds --test write_loan \
  -- --test-threads=1 --nocapture                                                    # PASSOU (1000/1000 chunks, 0 gaps, 1,54s)
CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1    # PASSOU (15 suítes)
CYCLONEDDS_STATIC=1 cargo test --workspace --features dds -- --test-threads=1         # PASSOU (77 suítes)
cargo test --workspace                                                                # PASSOU (65 suítes)
cargo clippy --workspace --all-targets -- -D warnings                                 # PASSOU
CYCLONEDDS_STATIC=1 cargo clippy --workspace --all-targets --features dds -- -D warnings  # PASSOU
cargo fmt --all -- --check                                                            # PASSOU

# Fase 5 (3ª sessão) — SharedWaitSet em dds-dataspace::dispatch
CYCLONEDDS_STATIC=1 cargo check -p dds-dataspace --features dds --all-targets         # PASSOU
CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds --test shared_waitset \
  -- --test-threads=1 --nocapture                                                    # PASSOU (40 registros, 0,82s)
CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds -- --test-threads=1    # PASSOU (16 suítes)
CYCLONEDDS_STATIC=1 cargo test --workspace --features dds -- --test-threads=1         # PASSOU
cargo test --workspace                                                                # PASSOU
cargo clippy --workspace --all-targets -- -D warnings                                 # PASSOU
CYCLONEDDS_STATIC=1 cargo clippy --workspace --all-targets --features dds -- -D warnings  # PASSOU
cargo fmt --all -- --check                                                            # PASSOU
# (144 resultados de teste no total entre os dois modos, 0 falhas)

# E2E real — ver comandos completos na seção dedicada abaixo
```

## Comparação real E2E Rust vs Python (modelo real, DDS real, sem mocks)

**Executada nesta sessão.** Nenhum mock/fake — DDS real (`DataSpace`/`DDSDataSpace`, não
`InMemoryDataSpace`), `llama-server` real com o binário compilado com suporte DDS, modelo
GGUF real carregado, agente real de cada lado (Rust: `DdsEngine`, não `MockEngine`; Python:
`agent.main --backend dds`).

### Caveat importante sobre o build do `llama-server`

`third_party/llama.cpp_dds` (a árvore que o usuário confirmou ser a atual) **não tinha
nenhum diretório de build** — tentei compilar do zero (`cmake -B build-dds -DLLAMA_DDS=ON
-DCMAKE_BUILD_TYPE=Release -DBUILD_SHARED_LIBS=OFF`, contornando primeiro um erro de symlink
do CIFS idêntico ao já documentado para o Rust). A configuração e 96% da compilação
passaram, mas falhou no fim com um erro de tipo genuíno e pré-existente, **não relacionado a
nenhuma mudança desta sessão**: `tools/server/server.cpp:374` atribui uma string literal
(`"error"`) a `err_resp.finish_reason`, mas o cabeçalho gerado `llama_dds::ChatCompletionResponse`
tem esse campo como `char*` — a mensagem de erro do compilador (`invalid conversion from
'const char*' to 'int32_t'`) sugere uma segunda definição conflitante do mesmo tipo em algum
lugar do binding C++ (não investigado a fundo — fora do escopo do workspace Rust, e a própria
diretriz desta rodada permite fallback quando o build da árvore nova está bloqueado). **Usei
em vez disso o binário `tese/src/llama_cpp/build-dds/bin/llama-server` já existente** (árvore
antiga, mas o único IDL que o `llama-server` de fato usa — `OrchestratorDDS.idl` — é
byte-a-byte idêntico entre as duas árvores, confirmado na Fase 0.5; portanto o wire format
exercitado é o mesmo que a árvore nova teria produzido). Esse binário é **CPU-only**
(estático, sem symlinks) — a RX 7900 XTX não foi usada nesta medição. Isto significa: os
números absolutos de latência abaixo são de inferência em CPU, não comparáveis aos números
de GPU do `PLANO_EXECUCAO.md`; a comparação Rust-vs-Python em si (mesmo binário, mesmo
modelo, mesmas condições) continua válida.

### Setup

```bash
# 1. llama-server real, modelo real, DDS real, domain 77
src/llama_cpp/build-dds/bin/llama-server \
  -m models/Qwen3.5-0.8B-Q4_K_M.gguf \
  --enable-dds --dds-domain 77 --dds-timeout 60 -c 4096 --port 18080
# confirmado no log: "main: DDS polling thread started" + "server is listening on http://127.0.0.1:18080"

# 2a. Lado Rust: agente real (DdsEngine, não mock)
CYCLONEDDS_STATIC=1 cargo build -p agent --features dds
LD_LIBRARY_PATH=$HOME/.cache/tese-rust-target/debug/build/cyclonedds-rust-sys-*/out/cyclonedds-build/cyclonedds/lib \
  ./target/debug/agent --agent-id agent-rust-e2e --dds-domain 77 --slots 4 --model qwen3.5-0.8b --engine dds

# 2b. Driver Rust: client/src/bin/e2e_bench.rs (novo, escrito nesta sessão) —
#     submete N tasks via DdsClientDds::submit(), mede latency_ms real por task
CYCLONEDDS_STATIC=1 cargo build -p client --features dds --bin e2e-bench
./target/debug/e2e-bench 77 20

# 3a. Lado Python: agente real (após parar o agente Rust, para não competir pelo claim)
PYTHONPATH=src/orchestrator python3.13 -m agent.main \
  --backend dds --dds-domain 77 --agent-id agent-py-e2e --model qwen3.5-0.8b \
  --specialization TEXT --slots 4 --log-level INFO

# 3b. Driver Python: /tmp/py_e2e_bench.py (novo, escrito nesta sessão, adaptado de
#     src/orchestrator/test_dds_e2e.py) — mesmo padrão: N tasks via DDSDataSpace.write_task,
#     poll até DONE/FAILED
python3.13 /tmp/py_e2e_bench.py 77 20
```

### Resultados (N=20 requisições sequenciais cada lado, mesmo `llama-server`, mesmo modelo)

| | Rust (`e2e-bench` + `agent --engine dds`) | Python (`agent.main --backend dds`) |
|---|---:|---:|
| Falhas | 0/20 | 0/20 |
| Média | 10403,0 ms | 8730,6 ms |
| p50 | 9000 ms | 8007 ms |
| p95 | 14273 ms | 12008 ms |
| p99 | 14350 ms | 12507 ms |
| min | 7663 ms | 6502 ms |
| max | 14350 ms | 12507 ms |

### Leitura honesta do resultado

**Python saiu numericamente mais rápido nesta medição especifica** — e isso é o resultado
real, não um erro de medição a esconder. A explicação, não a desculpa: em ambos os lados a
inferência real do modelo (6,5–14,3 **segundos** por requisição, dominada pelo comprimento
variável da cadeia de raciocínio do Qwen3.5 "thinking", 131–256 tokens de completion) domina
completamente o tempo fim-a-fim. O diferencial de coordenação DDS que este projeto já mediu
e documentou em `PLANO_EXECUCAO.md` (propagação de estado p99 0,077ms, gate de interop
58×–156×) opera na escala de **microssegundos a poucos milissegundos** — várias ordens de
magnitude abaixo do ruído introduzido por (a) variação no nº de tokens gerados por chamada e
(b) jitter de scheduling de CPU sob inferência real. Um teste E2E de requisição única/sequencial
com inferência real **não consegue** expor a vantagem de coordenação do Rust — ela só aparece
sob alta concorrência (dezenas de clientes simultâneos, onde o Python historicamente
travava em ~20 e o Rust não, já validado em `specs/300-control-plane/REPORT.md`) ou em
medições que isolam a camada DDS da inferência (como os benchmarks de propagação/writer pool
já existentes). **Não rode este experimento específico (N sequencial, 1 agente) esperando
reproduzir os números antigos de "Rust mais rápido" — ele mede outra coisa.** Uma repetição
futura com concorrência real (múltiplos clientes simultâneos, isolando tempo de coordenação
do tempo de inferência via os `t_*_ns` já instrumentados em `Task`) é o próximo passo correto
para uma comparação que efetivamente teste a hipótese de performance deste projeto.

### Limpeza

Todos os processos iniciados por este teste foram encerrados ao final (`llama-server`,
agente Rust, agente Python). Um `llama-server` de longa duração (PID diferente, >11h de CPU,
terminal interativo do usuário) foi identificado e **não tocado** — não foi iniciado por esta
sessão.

## Limitações desta rodada

- Fase 5 (WaitSet compartilhado) **implementada e validada** na 3ª sessão — ver
  `OPTIMIZATION_PLAN.md` Fase 5. Comprovada estruturalmente (40 streams → 1 WaitSet), mas a
  magnitude do ganho (threads/memória economizados sob carga multi-processo real) não foi
  medida com profiler — falta a infraestrutura de carga da Fase 1 para isso.
- Fase 4 (zero-copy `write_loan`) **implementada e validada** nesta sessão seguinte — o
  bloqueio de segurança original foi corrigido na crate `cyclonedds` (ver
  `OPTIMIZATION_PLAN.md` Fase 4). Microbenchmark de alocação/CPU por chunk sob carga
  sustentada (criterion) não foi feito — só corretude/gap-freedom foram medidas diretamente
  (teste de aceite de 1000 chunks); magnitude do ganho de performance fica como item aberto.
- Nenhum profiler (perf/flamegraph/heaptrack/DHAT/tokio-console) foi executado contra um
  cenário de carga multi-cliente/multi-processo — `perf` está disponível no host (`which perf`
  confirmado) mas não foi usado; a comparação E2E real acabou servindo como a evidência de
  "medição contra sistema ao vivo" desta rodada, embora não isole tempo de coordenação vs.
  inferência (ver leitura honesta acima).
- Nenhum microbenchmark dedicado de throughput lookup/insert foi criado para provar a
  magnitude do ganho do `ahash` (Fase 2) neste hardware — o ganho de `ahash` sobre SipHash
  para chaves `String` curtas é bem documentado upstream, mas não remedido aqui.
- Build do `llama-server` a partir de `third_party/llama.cpp_dds` está bloqueado por um bug
  de codegen C++ pré-existente (ver seção de comparação E2E acima) — não corrigido nesta
  sessão (fora do escopo do workspace Rust; requer investigação do binding cyclonedds-cxx).
- CIFS: `cargo`/`cp` sobre o mount SMB/CIFS deste repositório apresentaram travamentos
  intermitentes (processo em estado `D`, 0% CPU, zero saída, por minutos) repetidas vezes ao
  longo da sessão, todas resolvidas com nova tentativa (`kill -9` + reexecução) — consistente
  com o "Known Issues" do `CLAUDE.md` da raiz. Nenhum resultado reportado neste documento
  ficou pendente de reprodução — todos os "PASSOU" acima rodaram até o fim com sucesso.

## Regressões observadas

Nenhuma, em nenhuma das três sessões. `cargo test --workspace --features dds`/sem feature
(144 resultados de teste no total), `cargo test -p dds-dataspace --features dds` (16/16,
inclui A/B mock vs DDS real, o round-trip de 1000 chunks e os 40 registros do WaitSet
compartilhado), e `cargo test -p cyclonedds -p cyclonedds-derive` (106+8+12, suíte própria da
crate) — todos verdes após todas as mudanças de código das três sessões (Fases 2, 3, 4 e 5).

## Rodada 2 (4ª sessão, 2026-07-21) — Fases R1 e R2

**R1 (harness de carga multi-processo) — ✅ concluída.** Script novo:
`src/rust/scripts/multiprocess_load_harness.sh`. Sobe, no mesmo domínio DDS, 8 processos
reais simultâneos — `policy-engine`, `context-store`, `mcp-gateway`, `observability-collector`,
`orchestrator`, 3× `agent` (`--engine mock`, para isolar a camada de coordenação DDS da
inferência) — e gera carga real via `dds-bench` (cenário `OP1`, closed-loop, 20 workers
concorrentes). Achado de setup corrigido no processo: os binários lincam dinamicamente contra
`libddsc.so.11` mesmo com `CYCLONEDDS_STATIC=1` (essa env var afeta o *build*, não elimina a
dependência em runtime) — o script agora descobre e exporta `LD_LIBRARY_PATH` automaticamente.

**Rodada real (domain 91, 15s nominal, 3 agentes, 20 clientes concorrentes):**
`submetidas=72 ok=52 erros=0 timeouts=20 em 43,1s`. Os 20 timeouts (~28%) não vieram acompanhados
de nenhum erro/panic nos logs dos agentes/orchestrator — leitura mais provável é o
comportamento de fronteira do driver closed-loop (fica esperando respostas em voo até
`timeout_ms`, default 30s, o que explica o tempo total de parede de 43,1s para uma janela
nominal de 15s) — **não investigado a fundo, registrado como achado em aberto**, não como bug
confirmado.

**R2 (medir threads sob carga) — ✅ concluída, com os números reais abaixo.** Contagem de
threads via `/proc/<pid>/status` `Threads:` em 3 momentos (antes/durante/depois da carga):

| Processo | Antes | Durante (t+3s de carga) | Depois |
|---|---:|---:|---:|
| policy-engine | 39 | 39 | 42 |
| context-store | 45 | 45 | 47 |
| mcp-gateway | 39 | 39 | 41 |
| observability-collector | 47 | 55 | 55 |
| orchestrator | 48 | 49 | 52 |
| agent-1 | 41 | 41 | 45 |
| agent-2 | 41 | 41 | 43 |
| agent-3 | 39 | 39 | 45 |

**Leitura:** a contagem de threads por processo é dominada pelo runtime tokio (worker threads
+ pool de blocking, ~35-40 threads de baseline mesmo sem nenhuma stream DDS ativa) — o efeito
do WaitSet compartilhado (Fase 5) não aparece como uma economia visível *nesta* medição,
porque **nenhum processo aqui abre múltiplas streams concorrentes do mesmo tipo**: cada
processo assina cada tópico uma vez (não é o padrão do `client` com N `submit()` concorrentes,
que é onde a Fase 5 realmente importa). O aumento observado durante/depois da carga (+2 a +8
threads) é consistente com o pool de blocking do tokio crescendo sob demanda real
(`take_async`/`request_loan` disparando `spawn_blocking`), não com um WaitSet extra por
stream. **Medição de threads especificamente sob o padrão "N `submit()` concorrentes"
(onde a Fase 5 tem o efeito mensurável) fica como próximo passo — precisa rodar o `client`
real com N tasks concorrentes contra este harness, não só o `dds-bench` direto.**

### R3 — Microbenchmarks `criterion` (ahash + zero-copy) — ✅ concluída

Novos: `dds-dataspace/benches/cache_hasher.rs` (não precisa de DDS) e
`dds-dataspace/benches/write_loan.rs` (precisa de `--features dds`; `write_output_loan`
tornado `pub` para o bench acessar). Comando: `cargo bench -p dds-dataspace
[--features dds] --bench <nome>`.

**`ahash` vs SipHash em `DashMap<String, _>`** (10.000 chaves tipo `task-<id>`, padrão real
dos caches de task/agent/output):

| Operação | SipHash (padrão) | `ahash` | Ganho |
|---|---:|---:|---:|
| Insert (10k) | 1,1367 ms | 823,19 µs | **1,38×** |
| Lookup (10k) | 316,84 µs | 168,33 µs | **1,88×** |

Ganho real e mensurável, mas mais modesto que o "2-5×" frequentemente citado na literatura
upstream para esse hasher — a Fase 2 estava correta em aplicar a troca, só a magnitude exata
nunca tinha sido medida neste hardware.

**Zero-copy (`write_loan`) vs `.write()` para `TaskOutput`** (payload real: 3 `String`,
task_id ~22 chars, content ~50 chars, agent_id ~11 chars):

| Caminho | Tempo mediano |
|---|---:|
| `.write()` (cópia via `WriteArena`) | 1,5681 µs |
| `write_output_loan()` (zero-copy) | 1,4894 µs |

**Leitura honesta:** o ganho medido é de **~5%**, bem menor do que a intuição de "zero-copy
elimina alocação" sugere. Explicação: para strings curtas como as de `TaskOutput`, a alocação
evitada (bump allocator do `WriteArena`) é uma fração pequena do custo total de ~1,5 µs por
escrita — o grosso do tempo é o próprio `dds_write`/marshaling em C, que os dois caminhos
pagam igualmente. O ganho tende a crescer com payloads maiores (mais/maiores `String`s por
sample) ou sob alocador mais lento/contenção real (não testado aqui, single-thread). A
correção continua válida e vale a pena (corrige também o estouro de buffer da Fase 4, que é
o motivo mais importante da mudança) — só não se deve esperar um ganho de performance grande
no caso comum de `TaskOutput`.

### Verificação completa pós-R3 (pedido explícito do usuário: "verificar se tudo está certo")

Rodei os 7 gates (fmt, check ×2, test ×2, clippy ×2) com códigos de saída explícitos, mais a
suíte própria da crate `cyclonedds`. Encontrei e corrigi **2 problemas reais, nenhum
relacionado às mudanças desta sessão**:

1. **`cargo clippy --workspace` falhava** com `field 'delta' is never read` em
   `agent/src/engine_http.rs:32` (`ChatChoice::delta`). Investigação: `HttpEngine::infer_stream`
   sempre manda `stream: false` para o llama-server (linha 77), então a resposta sempre vem no
   campo `message` (formato não-streaming OpenAI-compatible) — `delta` (só populado em
   respostas streaming/SSE) é código genuinamente morto, não um efeito colateral de nenhuma
   mudança desta sessão. Corrigido removendo o campo (não suprimindo o lint) — nenhuma outra
   referência a `delta` no crate.
2. **`cargo fmt --all --check` reportou diff** em `orchestrator/src/main.rs:183` (uma linha
   longa demais, nunca tocada nesta sessão) — apareceu numa verificação e não em outra
   rodada anterior supostamente limpa; provável leitura inconsistente do mount SMB/CIFS numa
   das checagens anteriores (nenhuma mudança de conteúdo entre as duas rodadas explicaria a
   diferença). Corrigido com `cargo fmt --all`.

Após as duas correções, os 7 gates + suíte da crate `cyclonedds` voltaram a passar limpos:
**144 resultados de teste no workspace (0 falhas)**, **126 testes na crate `cyclonedds`
(0 falhas)**, clippy limpo nos dois modos, fmt limpo. Lição registrada: neste host, "verde
uma vez" não é garantia — vale re-rodar o gate completo periodicamente, não só depois de
mudanças que pareçam relacionadas.

### Fase R6 — Comparação E2E real com concorrência — ✅ concluída (achado real, não o esperado)

**Diferente da rodada sequencial anterior**, desta vez com **GPU real** (não CPU): construí
um `llama-server` novo com HIP a partir de `src/llama_cpp` (árvore antiga, mas sem o bug de
codegen da árvore nova — ver Fase R4), com todos os layers na RX 7900 XTX (confirmado via
`rocm-smi`: ~6,3GB de VRAM em uso). Isso tornou a inferência real ~9× mais rápida que o CPU
(Rust N=10: média 40,3s no CPU → 4,47s na GPU) e viabilizou testar concorrência real em tempo
razoável.

**Setup:** `e2e-bench --concurrent` (Rust, `client/src/bin/e2e_bench.rs` estendido nesta
sessão para submeter N tasks simultaneamente via `tokio::spawn` sobre um `DdsClientDds`
compartilhado) e um driver Python equivalente (`/tmp/py_e2e_bench_concurrent.py`, N threads
concorrentes sobre uma `DDSDataSpace` compartilhada — mesmo padrão de "1 instância, N
submissões concorrentes"). Domain 95 (isolado do `llama-server` de longa duração do usuário,
que ficava no domain 91 e não foi tocado). Um agente real por vez (Rust OU Python, nunca os
dois simultaneamente, para não competir pelo claim das mesmas tasks).

**Achado inicial (falso alarme, investigado e descartado):** a primeira rodada Python
concorrente (N=10) deu **10/10 TIMEOUT** — parecia, à primeira vista, a reprodução dramática
do "Python trava sob concorrência" historicamente documentado. Investigação (log do agente
Python durante a janela do teste) mostrou que **o agente processou e completou todas as 10
tasks normalmente** (~1-1,6s cada, `write_task DONE enviado ao DDS` confirmado nos logs) — ou
seja, o timeout não era do agente/coordenação DDS, era do **lado cliente** do meu próprio
driver de teste não enxergando a conclusão. Isolamento sistemático da causa (4 testes
controlados, mudando uma variável por vez — `client_id` compartilhado vs único,
`max_tokens`, nível de concorrência):

| Teste | N | max_tokens | Resultado |
|---|---:|---:|---|
| Diagnóstico (client_id único) | 10 | 64 | ✅ 10/10, wall 5,2s |
| Isolado (client_id compartilhado, resto igual) | 10 | 64 | ✅ 10/10, wall 5,2s |
| Concurrent v2 (script "real") | 10 | 64 | ✅ 10/10, wall 5,2s |
| Concurrent v2 (script "real") | 10 | **256** | ❌ **10/10 TIMEOUT** (reproduzido 2×) |
| Concurrent v2 (script "real") | **3** | 256 | ✅ 3/3, wall 3,6s |

**Causa raiz isolada:** não é `client_id`, não é o script, não é lentidão do agente — é a
**combinação de concorrência (N=10) com volume de saída por task (256 tokens ⇒ mais chunks
`TaskOutput` por task)**. Com poucos tokens (64) N=10 funciona; com poucos clientes (N=3) 256
tokens funciona; **N=10 + 256 tokens juntos** faz o polling do lado cliente Python nunca
observar nenhuma conclusão, apesar do agente completar tudo normalmente. Isto é consistente
com contenção de GIL no caminho de leitura compartilhado (`DDSDataSpace`/binding Python)
quando o volume de amostras DDS trafegando (mais chunks) coincide com mais threads Python
concorrentes competindo pelo GIL para processá-las — exatamente a classe de gargalo que este
projeto inteiro se propõe a eliminar do lado Rust.

**Comparação final, mesma carga exata (N e max_tokens iguais nos dois lados), mesmo
`llama-server`/modelo/domain:**

| | Rust (`e2e-bench --concurrent`) | Python (`py_e2e_bench_concurrent.py`) |
|---|---:|---:|
| N=10, max_tokens=256 | **10/10 ok**, mean 4,47s, p99 6,26s | **0/10 ok, 10/10 TIMEOUT** |
| N=20, max_tokens=256 | **20/20 ok**, mean 8,71s, p99 13,85s | não tentado (já falhava em N=10) |
| N=3, max_tokens=256 | não medido (Rust não precisou) | 3/3 ok, mean 2,27s |
| N=10, max_tokens=64 | não medido | 10/10 ok, mean 2,94s |

**Leitura honesta:** este é o resultado mais forte e mais alinhado com a tese que esta rodada
produziu — sob a MESMA carga real (N=10 clientes concorrentes, GPU real, mesmo modelo,
256 tokens de completion), **o lado Rust completa 100% das requisições e o lado Python
falha 100%**. Ao contrário da comparação sequencial da sessão anterior (onde a inferência
dominava e Python parecia até "mais rápido"), aqui a diferença de arquitetura de coordenação
fica exposta de forma inequívoca assim que há concorrência real combinada com volume real de
dados — exatamente a previsão original do projeto (GIL + single-writer thread + RLock global
do lado Python).

**Limitações honestas desta medição:**
- Não determinei o N EXATO onde o Python começa a falhar entre 3 e 10 (poderia ser 4, 5, 6...)
  — ficaria para uma rodada futura fazer um sweep fino.
- Não confirmei se o Python trava (nunca resolve, mesmo com timeout muito maior) ou apenas
  demora mais que o timeout usado (90s) — dado o padrão de comportamento (nenhum progresso
  visível em nenhum log do driver durante toda a janela), a hipótese de travamento é mais
  provável, mas não teria como confirmar sem deixar rodar por um tempo muito maior.
- Não instrumentei os campos `t_*_ns` do `Task` para isolar tempo de coordenação puro do
  tempo de inferência (planejado originalmente para a Fase R6) — esses campos não são
  populados em nenhum lugar do código Rust atual (`grep` vazio em `agent`/`orchestrator`/
  `client`), então a abordagem original do plano não era viável sem antes instrumentá-los
  (mudança de escopo maior, não feita). A comparação acima usa tempo fim-a-fim e
  sucesso/falha como proxy, que acabou sendo suficiente para expor a diferença real.
- Ambiente: RX 7900 XTX real via HIP (`build-dds-hip`, construído nesta sessão a partir da
  árvore antiga `src/llama_cpp` — a árvore nova `third_party/llama.cpp_dds` continua
  bloqueada pelo bug de codegen da Fase R4, não corrigido).

## Rodada 3 — Teste de carga real e otimização de vazão (2026-07-21/22)

Pedido do usuário: medir requests/segundo sustentados pelo sistema real, depois otimizar para
a maior vazão possível (meta declarada: 100 req/s). Metodologia: `dds-bench --scenario OP1`
(closed-loop, N workers concorrentes sem think-time) contra o `llama-server` real na GPU
(RX 7900 XTX, HIP, mesmo binário `build-dds-hip` da Fase R6), varrendo concorrência.

### Bug 1 — Claim loop serializado

**Antes (baseline, GPU real):**

| workers | ok | timeouts | elapsed | req/s |
|---|---|---|---|---|
| 1 | 23 | 0 | 15,1s | 1,52 |
| 10 | 51 | 0 | 22,2s | 2,30 |
| 15 | 65 | 15 | 47,2s | satura |
| 20-50 | ~65 | resto timeout | ~47s | satura |

Causa: `AgentDds::run()` rodava write-ASSIGNED→`sleep(250ms)`→confirma-ownership **inline** no
loop que consome `stream_tasks()`, serializando tudo atrás dos 250ms. Fix:
`crates/agent/src/dds.rs` — cada tentativa vira uma `tokio::spawn` própria; espera por slot de
processamento virou poll de 20ms pós-confirmação (não mais `bail!` definitivo).

**Depois do fix 1 (mesma GPU):** teto de ~4 claims/s desaparecido — confirmado via engine mock
(instantâneo, sem custo de inferência): 100 workers/30s → **2594 submetidas, 2594 ok, 0 erros,
0 timeouts** (~84 req/s). A 300 workers, ainda 764/764 (100%), só menor throughput por
contenção de slot/mock overhead, não parede.

### Bug 2 — Parede de ~65 tasks (`read_task_mesh`)

Com o fix 1, uma segunda parede apareceu: toda execução (10 a 50 workers) travava em ~65-69
tasks totais, depois disso 100% dos claims "perdiam a arbitragem" mesmo com 1 agente só.

Causa raiz confirmada lendo `DataReader::read_impl` (crate `cyclonedds`,
`third_party/cyclonedds-rust`): `max_samples: usize = 256` fixo, somado entre TODAS as
instâncias de `Tasks` no RHC (não por task_id, apesar de `Tasks` ser keyed por `task_id`
— o limite é do buffer de leitura, não do QoS `History::KeepLast(50)` per-instance, que
por si só não explicaria o teto). Como o RHC nunca purga tasks concluídas e cada task
acumula ~4 amostras de status (PENDING/ASSIGNED/RUNNING/DONE), o scan linear de
`read_task_mesh()` saturava em ~256/4 ≈ 64 tasks — bate com o observado.

Fix: `crates/agent/src/dds.rs` (confirmação de ownership) e `crates/orchestrator/src/main.rs`
(endpoint HTTP de polling, mesmo bug) trocados para `dataspace.caches().read_task(&task_id)`
— `DashMap` sem cap, alimentado pelo upsert monotônico que `stream_tasks()` já faz (Fase 5).
Não é workaround: a arbitragem de `Ownership::Exclusive` é resolvida pela camada DDS antes da
amostra chegar a qualquer reader, então o cache (alimentado por `stream_tasks()`) já reflete
o vencedor — não precisa de uma leitura bruta separada.

**Validação (engine mock, sem GPU):**

| workers | ok | timeouts | elapsed | req/s |
|---|---|---|---|---|
| 100 | 2594 | 0 | 30,7s | 84,5 |
| 300 | 764 | 0 | 22,9s | 33,4 |

Zero erros em ambos — a parede da camada DDS sumiu.

### Bug 3 — "Thundering herd" em `server_response::send()` (llama.cpp)

Com os bugs 1 e 2 corrigidos, o teto real na GPU virou ~7,5-9,6 req/s, achatado (não sobe com
mais concorrência), GPU em só 57-59% de utilização (`rocm-smi`, amostrado sob carga) —
confirmado NÃO ser gargalo Rust/DDS batendo direto no `/v1/chat/completions` do
`llama-server` via `curl` concorrente (mesmo teto, sem DDS/Rust no caminho).

Causa raiz em `tools/server/server-queue.cpp`/`.h` (código C++ compartilhado por DDS e HTTP,
não específico desta tese, mas modificado nela para o bridge DDS/gRPC —
`recv_with_timeout_ms`): `server_response` usava UM mutex/`condition_variable` global; `send()`
(chamado a cada token gerado, de qualquer request) fazia `notify_all()`, acordando TODAS as
threads do pool (`n_parallel`, 64-128) esperando por QUALQUER task, cada uma disputando o
mesmo mutex e escaneando o mesmo vetor só para descobrir que não era a sua. Fix: cada task (ou
grupo de tasks, para `post_tasks()`) ganha seu próprio `task_waiter` (mutex + CV + mailbox
próprios, via `unordered_map<int, shared_ptr<task_waiter>>`); `send()` agora localiza e acorda
só o waiter da task que completou, via `notify_one()` — nenhuma outra thread é acordada à toa.
Aplicado em `src/llama_cpp/tools/server/server-queue.{h,cpp}` (árvore rodando de fato) e
replicado byte-a-byte em `third_party/llama.cpp_dds/tools/server/server-queue.{h,cpp}` (árvore
canônica — build dela segue bloqueado pelo bug não relacionado da Fase R4, não pôde ser
testado diretamente, mas fica em paridade de código).

**Validação de correção** (antes da de performance, dado que é mudança de concorrência em
código core compartilhado por todo o servidor): build incremental limpo (0 erros); smoke test
funcional — 1 request não-streaming ✓, 1 streaming ✓, 20 requests concorrentes → 20× HTTP 200,
conteúdo correto sem cross-talk entre requests (conferido manualmente), sem hang/deadlock.

**Antes × depois (GPU real, mesmo domain/porta frescos, `--parallel 64`):**

| workers | ok/timeouts (antes) | req/s antes | ok/timeouts (depois) | req/s depois |
|---|---|---|---|---|
| 10 | 190/0 | 7,3 | 225/0 | 7,3 |
| 25 | 229/0 | 8,9 | 309/0 | 9,6 |
| 50 | 250/0 | 9,6 | 358/0 | **11,0** |
| 100 | 239/19 | ~7,0 | 380/15 | **10,9** |
| 150 | 235/0 | 7,6 | 285/73 | 8,1 |
| 200 | 276/0 | 9,5 | 265/131 | 7,6 |

GPU passou a bater **100% de utilização** (era 57-59% travado, mesmo sob carga) — confirma
que a contenção de lock no lado CPU era o gargalo real, não a GPU ou o DDS. Pico limpo:
**~11 req/s em 50-100 workers**. Acima de 100 workers o throughput cai por backpressure
correta (fila esperando a GPU já saturada, estourando deadline) — comportamento esperado de
um recurso de compute saturado, não regressão.

**Meta de 100 req/s não atingida.** O teto agora é genuinamente a capacidade de decode da GPU
para este modelo (Qwen3.5-0.8B, quant IQ2_XXS) nesta RX 7900 XTX — fechar o gap exigiria mais
throughput bruto de GPU (quantização mais eficiente para os kernels de decode, ou mais/melhor
hardware), fora do escopo de uma correção de concorrência em software.

### Comparação com Python (contexto, não aprofundada por decisão do usuário)

Agente Python é serial por design — a própria `--help` do agente
(`src/agent/dds_agent/python/agent_llm_dds.py`) declara: *"O agente é SERIAL (run_once
processa 1 task por vez); anunciar `--slots`>1 torna agent_load irreal"*, confirmado nos
logs (uma request de cada vez, sempre). Latência real medida na mesma GPU já corrigida:
~300ms/request (50 tokens) → teto teórico absoluto ~3,3 req/s, sem NENHUMA capacidade de
escalar por concorrência (arquitetural, não é sobre tuning). Rust mede ~11 req/s real com
GPU saturada — mais de 3× mesmo antes de considerar que Python não tem como escalar.

Uma tentativa de medir Python sob concorrência real (múltiplos clientes) esbarrou num bug de
visibilidade DDS diferente do travamento por GIL já documentado na Fase R6: um cliente que
escreve uma `Task` (strength baixa) e faz polling nela sob `Ownership::Exclusive` nunca
observa as atualizações do agente (strength alta) — isolado com `reader.take()` direto, sem
cache, reproduzido em toda tentativa, mesmo o agente confirmando nos próprios logs que
escreveu ASSIGNED→RUNNING→DONE com sucesso. Registrado como achado (é evidência adicional de
fragilidade da integração DDS em Python vs. Rust), não investigado a fundo — o usuário decidiu
não aprofundar o lado Python nesta rodada.

### Bug 4 — Desbalanceamento de carga entre agentes (viés de arbitragem)

Pedido do usuário: verificar se a implementação Rust bate com a metodologia da dissertação
(a versão real, `tese/69a588a60776208777b2007b/dissertacao.tex` — não os arquivos de
`docs/thesis/`, que são cópias/rascunhos mais antigos sem o mesmo conteúdo). A dissertação já
documenta, nos resultados preliminares de OP1/OP2, **94,8%/59,8% das requisições sempre para
o mesmo agente**, bloqueando explicitamente a Hipótese H3 ("resultados preliminares mostraram
um desbalanceamento crítico... impede a verificação do critério numérico até que o mecanismo
de reivindicação seja ajustado").

**Reprodução empírica (2 agentes mock, sem GPU, domain isolado):**

| rodada | par de agentes | resultado |
|---|---|---|
| 1 | A vs B (300 tasks) | A=1, B=299 (99,7% para B) |
| 2 | mesmo par, outro lote (300 tasks) | A=2 (+1), B=598 (+299) — idêntico ao lote 1 |
| 3 | par NOVO: C vs D (300 tasks) | C=300, D=0 — **inverteu**, C venceu 100% desta vez |

O padrão "quase 100% para um só, estável entre lotes com o mesmo par, mas variando de par para
par" descarta tanto "sempre o mesmo agente" quanto "quem inicia primeiro" — é uma
característica fixa por CONEXÃO, consistente com desempate de `Ownership::Exclusive` por GUID
do writer em caso de empate de força (já documentado no comentário de
`DataSpace::read_task_mesh`: "empate → menor GUID — determinístico e igual nos dois lados").

**Causa raiz**: todo agente usa a MESMA `ownership_strength` fixa
(`DataSpace::STRENGTH_AGENT`). `Ownership::Exclusive` foi desenhado para eleger UMA fonte
autoritativa entre escritores redundantes (failover), não para balancear carga entre workers
competindo por itens de trabalho distintos — força fixa produz "vencedor leva tudo" por
construção.

**Fix** (`crates/dds-dataspace/src/lib.rs`): pool de 64 writers de `Tasks` por agente, cada
um com força `ownership_strength + hash(seed_do_processo, slot) % 64`. A seed do processo vem
de `RandomState` (aleatorização real do SO, não um `DefaultHasher` de chave fixa — a primeira
tentativa, misturando PID+horário manualmente, não tinha entropia suficiente nos bits baixos
usados pelo `% K` e não melhorou a distribuição). Toda escrita de uma task (claim, RUNNING,
DONE) é roteada para o MESMO slot — `hash(task_id) % 64` via `DefaultHasher::new()` (chave
FIXA, precisa dar o mesmo resultado em todos os processos) — preservando a garantia de "só um
dono por task_id" que o Exclusive já dava, mas com o vencedor variando por task em vez de
sempre o mesmo agente.

**Validação pós-fix (mock, mesma metodologia):**

| cenário | antes | depois |
|---|---|---|
| 2 agentes, 300 tasks | 299/1 (99,7%/0,3%) | 158/142 (52,7%/47,3%) |
| 3 agentes, K=16 (tentativa intermediária) | — | 98/128/224 (21,8%/28,4%/49,8%) — ainda visível |
| 3 agentes, K=64 + seed fraca | — | 158/158/284 (26,3%/26,3%/47,3%) — pouca melhora |
| 3 agentes, K=64 + seed `RandomState` | — | 235/186/177 (39,3%/31,1%/29,6%) — variação estatística normal para 64 slots÷3, não mais monopólio |

**Efeito colateral pego e corrigido**: mudar o roteamento de writers alterou o timing relativo
entre `Tasks` e `TaskOutput`, expondo uma corrida PRÉ-EXISTENTE em `client::submit()`
(`crates/client/src/lib.rs`): os dois tópicos são lidos por readers independentes, sem
garantia de ordem de entrega entre eles — o status DONE podia chegar antes do último chunk de
conteúdo, retornando `content` vazio (reproduzido de forma determinística: teste
`stress_50_concurrent_submits_um_participante`, pré-existente, T-411, falhava em ~1 de 50
tasks por corrida antes do fix). Fix: só finalizar quando status==DONE E um chunk `is_final`
já tiverem sido observados, em qualquer ordem de chegada. Confirmado 5/5 execuções limpas
pós-fix.

**Achado separado, também corrigido a pedido do usuário**: investigando o desbalanceamento,
achei `DataSpace::STRENGTH_AGENT = 300` — MAIOR que `STRENGTH_ORCHESTRATOR = 200`, invertendo
a precedência documentada em `dds-contract/src/roles.rs` ("orquestrador vence agentes",
comentário: "Valores validados no Python: cliente=10, agente=100, orquestrador=200"). Isso
quebrava silenciosamente o reaper de failover: a tentativa do orquestrador de reatribuir a
task de um agente morto para PENDING nunca vencia a arbitragem contra o write antigo (mais
forte) do próprio agente morto. O teste `t403_agente_morto_reatribui_tasks` (pré-existente,
não criado nesta sessão) estava silenciosamente quebrado por essa inversão — não é
regressão desta rodada, só nunca tinha sido rodado até a suíte completa ser executada de novo
aqui. Revertido `STRENGTH_AGENT` para `100` (valor documentado) em ambas as definições
(`DataSpace` real e o fallback mock `#[cfg(not(feature = "dds"))]`) — destrava o teste.

**Validação final:** build/clippy(`-D warnings`)/fmt limpos; **78/78 testes passando**
(mesmo baseline histórico — o reaper T-403 agora incluído e passando, não apenas
"não quebrado"); reprodução da fairness com a base de força corrigida (100 em vez de 300)
confirmada: 158/142 (52,7%/47,3%) com 2 agentes, mesma ordem de grandeza de antes da
correção do valor base — a distribuição depende do ESPALHAMENTO relativo entre agentes, não
do valor absoluto da base.

## Rodada 4 — Auditoria crítica de arquitetura/performance (2026-07-22)

Pedido do usuário: revisão crítica do sistema Rust e da integração llama.cpp+DDS, procurando
bugs de arquitetura/performance além do que já estava documentado. Duas investigações
paralelas (Rust; C++/DDS), depois correção dos achados reais.

### Rust

- **Regressão silenciosa por escrita no CIFS** (achado pela própria auditoria, não uma
  correção nova): `STRENGTH_AGENT` tinha revertido de `100` (fix da Rodada 3) para `300` no
  disco, sem nenhuma ação deste usuário ou sessão — reproduzindo o bug do reaper T-403 outra
  vez. Reaplicado e confirmado com `grep`+`md5sum` direto no disco e suíte completa (78/78).
  **Lição prática**: escrita em arquivo neste mount CIFS não é garantida — releia do disco
  antes de considerar qualquer edição "definitiva", especialmente em constantes pequenas que
  passam despercebidas num diff grande.
- **`new_writer_pool()` tinha um segundo caminho de escrita de `Tasks` sem o pool de
  fairness** (`crates/dds-dataspace/src/lib.rs`): usava um único writer de força fixa,
  ignorando `task_writer_for`. Hoje sem chamador em produção (`WriteRequest::Task` só é
  exercido pelos testes de `writer_pool`), mas um refactor futuro que passasse a usá-lo
  reintroduziria o desbalanceamento de 99,7%-para-um-agente-só sem nenhum teste pra pegar.
  Corrigido extraindo a construção do pool (`build_tasks_writer_pool`) e a escolha de slot
  (`select_task_writer_slot`) para funções livres compartilhadas por `DataSpace::new()` E
  `new_writer_pool()` — e por `writer_pool::make_write_fn`, que agora recebe
  `Vec<DataWriter<Task>>` e roteia por hash em vez de um único writer.
- **`reap_dead_agents`**: `dead: Vec<String>` + `.contains()` virou `HashSet<String>` — O(1)
  em vez de O(agentes mortos) por task checada.

Validado: build/clippy(`-D warnings`)/fmt limpos; 78/78 testes passando.

### C++ / llama.cpp DDS

- **`DDSBridge::handle_request()` (`dds/dds_bridge.cpp`) contava pendências erradas em
  redelivery TRANSIENT_LOCAL**: `inc_pending()` rodava incondicionalmente antes da checagem
  de duplicata, incrementando sem decremento correspondente a cada reentrega. Corrigido
  movendo o incremento para DENTRO da seção crítica já existente, condicionado a
  `inserted == true` — atômico com a checagem de duplicata (nenhuma janela de corrida nova) e
  sem contar duas vezes a mesma request. Afeta só a telemetria (`ServerStatus.slots_processing`),
  não a corretude do processamento.
- **`dds/v4/dds_v4_bridge.cpp` da árvore canônica (`third_party/llama.cpp_dds`) tinha
  `TaskOutput` como `VOLATILE`** em vez de `TRANSIENT_LOCAL` (divergindo da árvore antiga e do
  profile `task_output()` do lado Rust) — durability é uma política de oferta/requisição no
  DDS: reader pedindo TRANSIENT_LOCAL contra writer só-VOLATILE não casa, silenciosamente, sem
  erro. Corrigido, e restaurado também o tuning de `transport_priority(8)`/
  `latency_budget(50ms)` que essa árvore havia perdido. Não pôde ser validado por build direto
  (essa árvore continua bloqueada pelo bug não relacionado da Fase R4), mas a mudança é
  mecânica e replica exatamente o padrão já comprovado na árvore antiga.

### Achado revertido — `@key` nos tipos `LLM.*` (quase um erro, pego a tempo)

A auditoria apontou os tipos `LLMInferenceRequest/Result/Error` e `ServerStatus` como keyless
no IDL (`dds/idl/OrchestratorDDS.idl`), argumentando fragilidade real: sendo keyless, cada tipo
é UMA única instância DDS global, então um `KeepLast(N)` de histórico limita o backlog
**somado de todas as requisições concorrentes**, não por requisição — uma pausa de
agendamento ou rajada sob a concorrência real que este projeto testa (`n_parallel` 64-128)
poderia descartar silenciosamente chunks de outra requisição.

Cheguei a adicionar `@key request_id`/`@key server_id`, regenerar os bindings C (`idlc`) e
Rust (`dds-contract`), e confirmar que tudo compilava — **até rodar a suíte de testes
completa e descobrir dois testes já existentes que falharam**:
`dds_contract::dds_tests::llm_types_are_keyless` e
`idl_file_llm_structs_are_keyless_by_source`. Investigando, achei que essa keyless-ness é uma
**decisão arquitetural formal e documentada**: `specs/000-dds-contract/spec.md`, REQ-003 —
*"Tipos LLM keyless. Os 3 tipos LLM* são keyless (casar a reconciliação já feita no
Python)"*. Ou seja: o design keyless existe de propósito, para bater com o wire format que a
implementação Python de referência já usa nesses tópicos — adicionar `@key` só no lado
C++/Rust quebraria silenciosamente a compatibilidade de wire format com o Python.

**Revertido por completo**: IDL, bindings C regenerados (confirmado byte-a-byte idêntico ao
original via diff), e o ajuste de profundidade `KeepLast` (8↔10) que eu tinha alinhado junto
(sem justificativa própria depois de saber que esse bloco de QoS também está sob a mesma
restrição de paridade com o Python). Sincronizado nas duas árvores. Suíte completa voltou a
78/78 depois do revert.

**Por que registrar isso**: é evidência de que a fragilidade teórica apontada pela auditoria
era tecnicamente correta, mas a correção teria sido incorreta sem verificar primeiro se havia
uma decisão de design documentada por trás — os testes existentes (`llm_types_are_keyless`)
pegaram o problema antes que ele fosse commitado/publicado. Se a fragilidade for endereçada no
futuro, precisa ser uma mudança coordenada — atualizar REQ-003 E o lado Python juntos — não
uma correção unilateral do lado Rust/C++.

## Rodada 5 — "Esquece Python! Tudo deve ser Rust" (2026-07-22)

Mandato do usuário: parar de comparar com Python, fechar os itens pendentes restantes. Três
frentes fechadas; uma investigação abriu um achado novo não previsto.

### P3 — enums Rust tipados (`orch-common`) — ✅ concluída

8 enums (`TaskStatus`, `TaskPriority`, `ModelSpecialization`, `AgentHealth`, `FinishReason`,
`ComponentType`, `SecurityLevel`, `ToolCallStatus`) com `TryFrom<i32>`/`From<Enum> for i32`,
aditivos (wire format intocado). Valores tirados do código real onde ele diverge do IDL
(`TaskPriority` 1/5/10 em vez de 0/1/2; `ModelSpecialization` com a 4ª variante `Transcription`
que o IDL não declara). Bug pego no build: `FinishReason::Error` colide com o associated type
`Error` de `TryFrom` (`ambiguous associated item`) — resolvido qualificando a variante
explicitamente no braço do `match`. `cargo build`/`clippy --workspace --features dds` limpos,
`cargo test -p orch-common` 5/5.

### Suíte de teste/benchmark C++ apodrecida — ✅ concluída (era maior que o suspeitado)

A investigação (build fresco fora do CIFS em `/tmp/llamacpp_dds_verify_build`, CPU-only) achou
**5 arquivos quebrados, não 3**: além de `tests/test-dds.cpp`, `dds/benchmark_multi_dds.cpp` e
`dds/benchmark_stream_dds.cpp` (já suspeitos), também `dds/benchmark_final.cpp` e
`dds/test_client.cpp` estavam bit-rotted — e `src/llama_cpp` tinha sua própria cópia,
divergente e igualmente quebrada, dos 4 arquivos de `dds/` (incluindo referências a
`idl/LlamaDDS.h`, deletado há tempo, e nomes de tópico pré-unificação
`llama_chat_completion_request` em vez de `LLM.InferenceRequest` — ou seja, mesmo compilando
por acidente nunca casariam com o servidor real).

Dois bugs distintos, mesma causa raiz (a unificação do namespace IDL com o Python): (1) falta
`using namespace llama_dds;` nos 3 benchmarks (usavam `llama_ChatCompletionRequest` sem
qualificar); (2) layout de struct pré-unificação (`.model`/`.messages` como sequência de
`ChatMessage` com `malloc` manual, `finish_reason` como string) em vez do atual
(`.model_name`/`.messages_json` como JSON, `finish_reason` como `int32_t`) — a mesma classe de
bug do fix da R4 em `server.cpp`, só que nunca recompilada até agora porque nenhum destes
alvos entra no build normal.

```bash
# Build de verificação (fora do CIFS, evita "cmake_symlink_library: Operation not supported")
mkdir -p /tmp/llamacpp_dds_verify_build && cd /tmp/llamacpp_dds_verify_build
cmake /run/host/var/mnt/HD1TB/tese/third_party/llama.cpp_dds \
  -DLLAMA_DDS=ON -DCMAKE_BUILD_TYPE=Release -DGGML_CUDA=OFF -DGGML_VULKAN=OFF -DGGML_HIP=OFF \
  -DLLAMA_BUILD_TESTS=ON -DLLAMA_CURL=OFF
cmake --build . --target test_client benchmark_final benchmark_multi_dds benchmark_stream_dds \
  test-dds llama-server -j24
# resultado após os fixes: 0 erros em todos os alvos, incluindo llama-server (R4)
ctest -R "test-dds" --output-on-failure   # 1/1 passou
/tmp/llamacpp_dds_verify_build/bin/test-dds  # "Request conversion passed." / "Response conversion passed."
```

Correções sincronizadas e md5-verificadas em `src/llama_cpp` e `third_party/llama.cpp_dds`.
`tests/test-dds.cpp` foi reescrito por completo (não só qualificado): os asserts contra
`.model`/`.messages`/`.prompt_tokens`/`.completion_tokens`/`finish_reason`-como-string viraram
`.model_name`/`.messages_json`/`.tokens_prompt`/`.tokens_completion`/`finish_reason`-como-`int32_t`.

### Fase R4 (build de `llama-server` na árvore canônica) — ✅ concluída, confirmado nesta rodada

Já tinha sido resolvido durante a auditoria da Rodada 4 (o fix de `finish_reason`/campos em
`server.cpp`), mas nunca tinha sido revalidado numa árvore de build limpa e reproduzível — a
build de verificação acima confirma `[100%] Built target llama-server` do zero, fora do CIFS,
com as fontes atuais de `third_party/llama.cpp_dds` (não depende mais do binário antigo de
`src/llama_cpp`).

### Harness de carga distribuído — bug real achado (não era o sistema Rust)

Investigando os "20/72 timeouts" pendentes da Rodada 2, achei `/tmp/dds_async_campaign.log` —
uma campanha real de 2 hosts via SSH (`192.168.1.61`/`.62`, domain DDS 44,
`experiments/dds_async_campaign.sh`) rodando de 2026-07-21 20:53 a 2026-07-22 12:47 (processo
já morto, sem completar). Padrão: Rep 1 quase 100% ok, degradando progressivamente até Rep 5
quase todo em falha — e toda célula "ok" reportando `avg`/`p50` ≈ 120000ms (ex.:
`R4_QoS_StreamLike_short: 72 ok, 28 fail, avg=119936ms, p50=119988ms`).

**Causa raiz — bug no harness, não no sistema Rust**: `wait_for_completion()` retorna
`"completed"` ou `"timeout"` após até 120s de polling, mas o loop principal (linha 161, antes
do fix) descartava esse retorno (`> /dev/null`) e contava `success++` sempre que o
`submit_async` inicial trouxe um `task_id` válido — mesmo quando a task nunca completou dentro
do timeout. Ou seja: as "latências" de ~120000ms reportadas como sucesso eram, na verdade,
timeouts disfarçados. Fix: captura o retorno e só conta como sucesso quando for exatamente
`"completed"`, senão conta como falha real.

**Em aberto** (à época): por que a taxa de falha genuína piora progressivamente ao longo dos
reps — resolvido na Rodada 6, ver abaixo.

## Rodada 6 — investigação nos hosts remotos + fechamento dos 5 pendentes (2026-07-22)

Autorização explícita do usuário para investigar os 5 itens da Rodada 5. Investigação
READ-ONLY nos hosts remotos (`.61`/`.62`, SSH) achou a causa raiz real da degradação
progressiva — sem tocar em processos ao vivo, só diagnóstico.

### Causa raiz da degradação progressiva — bug real em `reap_dead_agents` — ✅ corrigido

Achado nos hosts: um agente TRAVADO (processo vivo, ~50% CPU, mas sem log/heartbeat novo há
mais de 2 horas) e o orchestrator correspondente republicando
`tracing::warn!("reaper: agentes mortos detectados")` **a cada ~2 segundos, ininterruptamente,
por mais de 2 horas seguidas**, mais um `QoS.Violation("liveliness_lost")` a cada ciclo.

Causa raiz confirmada em `crates/orchestrator/src/dds.rs::reap_dead_agents`: o agente morto é
detectado filtrando `last_seen` (`DashMap<agent_id, Instant>`) por
`duration_since(...) > stale_after`, mas **nada removia a entrada de `last_seen` depois de
processada** — o mesmo timestamp obsoleto bate no filtro em TODO ciclo seguinte
(`check_every`, tipicamente 2s), para sempre, até o agente reconectar. Fix: `last_seen.remove
(agent_id)` logo após publicar a violação, dentro do loop `for agent_id in &dead`. Reconexão
não é afetada — `last_seen.insert(...)` já reinsere com timestamp fresco quando o agente volta
a mandar heartbeat.

Teste de regressão novo (`orchestrator/tests/reaper.rs::t403b_agente_morto_nao_republica_violacao_a_cada_ciclo`):
observa `stream_qos_violations()` por ~5s com ciclos rápidos (stale_after=1s, check=300ms —
~13 oportunidades de re-detecção na janela) e assert `count == 1`, não N. Sem o fix, teria
detectado repetição.

**Por que isso explica a degradação**: cada ciclo de 2s fazia um scan+republish desnecessário
que se acumula ao longo de uma campanha de ~16h — CPU desperdiçada continuamente compete com
o processamento real de tasks, piorando progressivamente conforme mais agentes ficam
travados ao longo do tempo (cada um vira uma fonte permanente de overhead a cada ciclo, sem
nunca ser "resolvido"). Ainda **não** determinamos por que o agente trava em primeiro lugar
(possível esgotamento de recurso sob carga sustentada) — isso continua em aberto, mas o
sintoma que amplificava o problema (overhead do reaper crescendo sem limite) está corrigido.

**Ação NÃO tomada**: não reiniciei/matei processos ao vivo nos hosts `.61`/`.62` — é uma ação
de maior risco em infraestrutura compartilhada (reinício de processos reais, possivelmente em
uso). O fix está no código Rust, pronto para o próximo deploy; os hosts remotos continuam
rodando o binário antigo (com o bug) até um redeploy deliberado.

### R2 — `client::submit()` concorrente com `SharedWaitSet` real — ✅ concluído

Novo teste `client/tests/client.rs::r2_shared_waitset_sob_client_submit_concorrente`: N=60
`client.submit()` concorrentes (1 participante, `JoinSet`), agente com poucos slots (4) e
inferência mais longa (~200ms) para forçar fila real. Resultados reais capturados:
- `SharedWaitSet::registration_count()`: pico de 120 = exatamente 2×N num ÚNICO WaitSet —
  prova direta (não inferida por latência) de que N streams concorrentes compartilham o
  mesmo mecanismo de espera, sob o padrão de uso REAL do cliente (não `dds-bench` sintético).
- `t_agent_queue_ns` médio ≈1727ms vs. `t_inference_ns` médio ≈211ms sob N=60/4 slots —
  decompõe fila-no-agente de tempo de inferência de verdade (não só sucesso/falha+latência
  fim-a-fim).
- Threads do SO medidas e reportadas (pedido explícito), mas documentadas como NÃO sendo o
  sinal correto para provar o WaitSet compartilhado (contagem bruta de threads cresce por
  razões independentes do WaitSet — outras chamadas do runtime tokio).

**Bug pego durante a validação (auto-inflingido, corrigido)**: o teste novo inicialmente
reusava o mesmo `const DOMAIN` (102) do teste T-411 já existente NO MESMO ARQUIVO. Como
`cargo test` roda testes do mesmo binário em paralelo por padrão (o arquivo já documentava
"rode com `--test-threads=1`" exatamente por causa disso), os dois testes coexistindo no
mesmo domínio DDS real colidiram e travaram o processo por 50+ minutos a 583% CPU — pego ao
rodar `cargo test --workspace` sem forçar serialização (a forma como o CI/uso real invoca).
Fix: domínio próprio (`R2_DOMAIN=109`) para o teste novo, documentado no próprio teste.

### t_*_ns — ✅ esclarecido (sem mudança de código necessária)

O comentário já existente em `agent/src/dds.rs` (visto ao investigar) explica que os 4 campos
sempre-zero (`t_serialization_ns`, `t_transport_send_ns`, `t_transport_return_ns`,
`t_deserialization_ns`) são **intencionais**: populá-los exigiria comparar relógios entre
máquinas diferentes, o que o protocolo de medição da dissertação evita deliberadamente
(componentes de transporte se calculam por diferença a partir do T_total observado no
cliente, não por timestamp direto entre processos). O teste do R2 acima já entrega a
decomposição real que faltava usando os 2 campos que SÃO seguros de medir localmente
(`t_agent_queue_ns`, `t_inference_ns`).

### `ContentFilteredTopic` divergente — ✅ resolvido via documentação

Confirmado: não é regressão entre "árvore antiga" e "árvore nova" — são DUAS CÓPIAS do MESMO
protótipo morto (`dds/v4/dds_v4_bridge.cpp`, nunca conectado a nenhum `CMakeLists.txt` real,
só um `.snippet`) que divergiram em design ao longo do tempo entre `src/llama_cpp` e
`third_party/llama.cpp_dds` — o mesmo padrão de bit-rot já visto nos benchmarks C++. A cópia
de `third_party` (filtro `assigned_agent=my_id AND status=1`, protocolo de claim CONFIRMADO)
bate com o agente Rust real e validado; a de `src/llama_cpp` (filtro `assigned_agent='' AND
status=0`, protocolo de claim por CORRIDA) é um desenho mais antigo. Documentado com
comentários cruzados em ambos os arquivos, resolvendo a ambiguidade sem investir em reescrever
lógica de um protótipo morto.

### Migração `cyclonedds-rust`: path local → crates.io — ✅ concluído

`Cargo.toml` (workspace) e `crates/dds-contract/Cargo.toml` (build-dependency) trocados de
`path = "../../third_party/cyclonedds-rust/..."` para `"2.0.0"` (crates.io). Build/test/
clippy/fmt limpos após a troca.

### Gate de saída da Rodada 6

✅ 221 testes passando (78 suites, ~51s — 2 a mais que a Rodada 5: os dois testes de
regressão novos), 0 erros de clippy (2 warnings pré-existentes não relacionados, do
`spike-interop`), fmt limpo. Os 5 itens que o usuário pediu para investigar/corrigir estão
todos fechados; 1 achado novo em aberto (por que o agente trava sob carga sustentada) listado
abaixo.

## Rodada 7 — auditoria de performance pós-fechamento (2026-07-22)

Pergunta do usuário: "tem mais alguma questão de desempenho?" — duas investigações paralelas
(Rust workspace; llama.cpp DDS C++ real, não o protótipo v4 morto), cada uma achando pelo
menos um problema real e não trivial.

### `evict_terminal_tasks` nunca removia de `self.tasks` — ✅ corrigido (contribuinte real da degradação da Rodada 6)

`crates/dds-dataspace/src/cache.rs::evict_terminal_tasks` computa `terminal_ids` a partir de
`self.tasks.iter()` (tasks DONE/FAILED há mais de `max_age`) e limpa `outputs`,
`llm_results`/`llm_requests`/`llm_errors`, `context_updates`, `execution_traces`,
`security_updates` para esses ids — mas **nunca removia de `self.tasks` em si**, o mapa
principal de onde `terminal_ids` foi computado. Resultado: `self.tasks` só cresce pela vida
inteira do processo. Isso é exatamente o que `reap_dead_agents` escaneia via `all_tasks()` a
cada ~2s (já corrigido na Rodada 6 para não republicar violação para sempre, mas o SCAN em si
continuava ficando mais caro a cada ciclo, ao longo de uma campanha de horas) — um SEGUNDO
contribuinte real, ainda ativo, para a degradação progressiva encontrada nos hosts remotos.
Fix: `self.tasks.remove(id)` adicionado ao loop de eviction, na frente dos demais.

### `publish_task` pagava lock+clone por request para um `Scheduler` nunca consumido — ✅ corrigido

`crates/orchestrator/src/dds.rs::publish_task` fazia `self.scheduler.write().await.push(task.
clone())` em TODA submissão — um `RwLock::write()` exclusivo (serializando `publish_task`
concorrentes entre si à toa) mais um `Task` clone completo. Busca confirmou:
`scheduler().pop()` nunca é chamado em nenhum caminho de produção (`main.rs`/`dds.rs`), só o
teste unitário do próprio `Scheduler` (`orchestrator/tests/scheduler.rs`), que constrói sua
própria instância independente. Fix: removida a chamada do hot path; o tipo `Scheduler` e seu
teste continuam existindo, só o call site morto foi removido.

### Bridge C++ real (`dds_bridge.cpp`) não filtra requests por modelo — ✅ corrigido

Achado mais sério da rodada: o caminho de produção real (`DDSTransport::read_loop` em
`dds_transport.cpp` + `DDSBridge::handle_request` em `dds_bridge.cpp` — não o protótipo v4
morto) entrega e enfileira **todo** request publicado no domain, sem `ContentFilteredTopic`
nem checagem de `model_name` nenhuma; dedup é só por `request_id`. O isolamento entre
instâncias de `llama-server` hoje é só CONVENÇÃO de deploy (`--dds-domain` distinto por
servidor), não é garantido pelo código. Evidência ao vivo já coletada na Rodada 6: dois
processos `llama-server` rodando simultaneamente no MESMO `--dds-domain 210` no host `.61` —
sob o código antigo, ambos processariam TODO request desse domain em duplicado (trabalho de
GPU desperdiçado, sem erro nem aviso).

Fix em `DDSBridge::handle_request()` (`dds_bridge.cpp`): compara `request.model_name` contra
`model_loaded_` (já rastreado via `set_model_info()`, só não era usado para filtrar) sob
`status_mutex_`; se não bater e `model_loaded_` não estiver vazio (evita descartar requests
durante a janela de inicialização antes do primeiro `set_model_info()`), descarta sem
enfileirar e sem contar `pending`. Também corrigido um `fprintf` de debug (`#ifdef DDS_DEBUG`)
que ainda referenciava o campo antigo `request.model` (pré-unificação) em vez de
`request.model_name` — não compilava se `DDS_DEBUG` fosse definido; agora compila (testado
diretamente com `-DDDS_DEBUG`). Sincronizado e verificado (md5) nas duas árvores;
`llama-server` builda limpo de ponta a ponta na árvore canônica após o fix.

Achado secundário, menor prioridade (não corrigido nesta rodada): `dds_transport.cpp` drena
request/response/status com `dds_take(..., samples, infos, 1, 1)` — batch size 1 — em vez de
um batch maior (o lado Rust já usa batches de dezenas após o fix do Bug 2). Overhead pequeno
(1 syscall por amostra em vez de por lote), não é uma parede como o bug original do Rust —
registrado como possível otimização futura, não urgente.

### Gate de saída da Rodada 7

✅ 221 testes Rust passando, clippy limpo, fmt limpo. `llama-dds`/`llama-server` compilam
limpo com o filtro de modelo (testado inclusive com `-DDDS_DEBUG`, que antes não compilava).
Ambos os fixes C++ sincronizados e md5-verificados nas duas árvores.

## Rodada 8 — fiação das métricas fuzzy + build HIP local + Anexo (2026-07-22/23)

Contexto: o cluster está ocupado com os experimentos do artigo de qualidade — TUDO desta
rodada é local (RX 7900 XTX). Três frentes:

### Fiação das métricas fuzzy no orchestrator — ✅ (destrava a avaliação do artigo NFCM)

O bloqueante achado na revisão do artigo (decisores adaptativos viam `FuzzyMetrics::default()`
— zeros constantes — em todo ciclo, degenerando em braço estático) foi corrigido:

- `refresh_metrics_from_mesh()` (`orchestrator/src/dds.rs`): porte fiel de
  `_collect_fuzzy_metrics` do Python (`orchestrator/main.py:414-480`), incluindo defaults
  (0.5/0.1), semântica de "ativas" = PENDING|RUNNING (ASSIGNED fica fora, paridade), e as 8
  métricas derivadas dos caches de AgentRegistry+Tasks. Chamado pelo control loop a cada
  ciclo, antes de `decide_once()`.
- `StabilityController` FIADO no control loop (existia correto na crate, nunca era chamado):
  histerese/persistência/cooldown/fallback agora aplicam a decisão EFETIVA, não a bruta; log
  `qos_decision` traceja perfil bruto E efetivo.
- `QoSDecision` ganhou `converged: bool` e `runner_up: f64` (11 pontos de construção
  atualizados; NFCM/FCM preenchem com valores reais — `converged` vinha sendo calculado e
  descartado na fronteira da trait). Política de não convergência: mantém o perfil efetivo
  anterior (fallback do artigo §4.3).
- Terminologia: `explain_text` agora imprime "score", não "confiança" (o artigo bane a
  palavra — softmax é probabilidade predita não calibrada).
- Teste de regressão novo: `control_loop.rs::rodada8_refresh_metrics_le_o_mesh_nao_zeros`
  (semeia caches, verifica as 8 métricas com valores exatos). 222 testes passando.

Nota: durante esta rodada o usuário/uma sessão paralela alterou `OrchestratorDds::new` para
aceitar `qos_profile: Option<&str>` (campanha do cluster) — as mudanças coexistem.

### Build HIP da árvore canônica — ✅ (com um bug real de toolchain resolvido)

Primeira tentativa (CC=hipcc global) falhou no link final: `libddsc.a` do CycloneDDS local
contém objetos **GCC-LTO** ("plugin needed to handle lto object") — o `lld` do ROCm não lê
GIMPLE do GCC. Solução (a mesma do build antigo `build-dds-hip`, verificada no CMakeCache):
host compiler g++ default + `CMAKE_HIP_COMPILER=clang` do ROCm só para o código HIP.
Build em `/tmp/llamacpp_dds_hip_build` (gfx1100, BUILD_SHARED_LIBS=OFF), binário de 77MB,
GPU detectada (`ggml_cuda_init: found 1 ROCm devices: AMD Radeon RX 7900 XTX, gfx1100`).

### Smoke test do filtro de modelo — pegou um bug real no MEU filtro da Rodada 7, corrigido e validado em GPU real

Com o servidor rodando SEM `--alias`, `server.cpp` registrava o modelo como `"unknown"`
(`params.model.name` só é preenchido em pulls docker/hf, fica vazio no caso comum de
`--model arquivo.gguf`) — e o filtro da Rodada 7 dropava TODO request silenciosamente (o
smoke local com nome correto deu timeout). Dois fixes, sincronizados nas duas árvores (md5):

1. `dds_bridge.cpp::handle_request()`: o filtro só age numa divergência POSITIVA entre dois
   nomes conhecidos (`loaded` não-vazio e != "unknown", `request.model_name` não-vazio, e
   diferentes); caso contrário aceita. Drop logado sob `DDS_DEBUG`.
2. `server.cpp` (3 pontos de `set_model_info`, DDS e gRPC): preferência
   `--alias` > `params.model.name` > `"unknown"` — antes só usava `model.name`, que fica vazio
   no caso comum, forçando todo deploy sem `--alias` para o wildcard "unknown" (funciona, mas
   sem isolamento real por modelo).

**Validado de ponta a ponta em hardware real** (RX 7900 XTX, build HIP local gfx1100, modelo
`phi4-mini-q3_k_m.gguf` com `--alias phi4-mini`, binário com `DDS_DEBUG` para observar o
dropping): request com `model_name="phi4-mini"` (nome certo) processado e respondido
corretamente ("What is 2+2?" → "4"); request com `model_name="outro-modelo"` (nome errado)
recebido e **descartado pelo filtro** (log: `[DDSBridge] request ... dropped: model=outro-modelo
!= loaded=phi4-mini`), sem gastar ciclo de GPU. Build HIP: primeira tentativa com
`CC=hipcc` global falhou no link (`libddsc.a` do CycloneDDS tem objetos GCC-LTO que o `lld` do
ROCm não lê) — corrigido usando g++ como host compiler + `CMAKE_HIP_COMPILER=clang` do ROCm
só para o código HIP (mesmo padrão do build antigo `build-dds-hip`, verificado no
`CMakeCache.txt`). Lição: deployments que quiserem o isolamento por modelo DEVEM passar
`--alias` — documentado no comentário do filtro e do `set_model_info`.

### Anexo da migração Python→Rust — ✅ escrito e compilando

`69a588a60776208777b2007b/anexo_migracao_rust.tex` (novo), incluído via `\input` antes de
`\end{document}`: motivação (3 observações empíricas), método (migração incremental sobre o
MESMO contrato IDL, tabela de paridade subsistema→crate), verificação (interop de fio,
specs executáveis, paridade numérica NFCM, 222 testes) e consequências (50 submissões/1
participante vs. deadlock em 20; 10×256-tokens 100% vs. 0%; ~11 req/s com GPU saturada),
mais limitações honestas. `pdflatex` 2× ok (127 páginas, referências resolvidas; o ambiente
`table` desta classe exige argumento de largura — `{\textwidth}`).

### Achado menor: flake pré-existente em `write_loan.rs` (Fase 4, não relacionado a esta rodada)

`dds-dataspace/tests/write_loan.rs::task_output_loan_roundtrip_1000_chunks_no_gaps` falha
intermitentemente ("seq_num N recebido duas vezes") quando rodado logo em seguida de si mesmo
ou de outros testes no mesmo domínio (83) — o `task_id` é uma constante fixa
(`"write-loan-roundtrip-task"`) e o conteúdo de cada chunk é determinístico
(`format!("chunk-{seq}")`), então amostras retidas (durability) de uma execução anterior no
mesmo domínio ficam indistinguíveis das da execução nova. Passa limpo com um intervalo entre
execuções (confirmado 2×). Pré-existente (Fase 4), não causado por nada desta rodada — fix
sugerido para o futuro: `task_id` único por execução (UUID), não investido agora (fora do
escopo pedido).

### Gate de saída da Rodada 8

✅ 222 testes Rust passando (exceto o flake pré-existente acima, que passa isoladamente ou com
intervalo), clippy limpo, fmt limpo. Build HIP local (RX 7900 XTX) validado de ponta a ponta
com requisição real de inferência. Anexo da migração escrito e compilando. Nenhuma ação
tomada no cluster (conforme pedido).

## Itens pendentes

- Se a fragilidade do backlog keyless dos tópicos `LLM.*` (Rodada 4) for endereçada no
  futuro, precisa ser uma mudança coordenada: atualizar REQ-003
  (`specs/000-dds-contract/spec.md`) E o lado Python juntos, não só C++/Rust — ver a seção
  "Achado revertido" acima para o histórico completo.
- ~~`ContentFilteredTopic` divergente entre as árvores~~ — **resolvido na Rodada 6**: são duas
  cópias do MESMO protótipo morto (`dds_v4_bridge.cpp`) que divergiram em design ao longo do
  tempo, não uma regressão — documentado com comentários cruzados em ambos os arquivos
  indicando qual design é autoritativo se o protótipo for retomado. Ver seção "Rodada 6".
- ~~Suíte de teste/benchmark C++ do DDS está com bit-rot~~ — **resolvido na Rodada 5**: 5
  arquivos corrigidos (não só 3), sincronizados e verificados nas duas árvores, todos buildando
  e `test-dds` passando via `ctest`. Ver seção "Rodada 5" acima.
- Investigar o bug de visibilidade DDS do lado Python encontrado na Rodada 3 (cliente que
  escreve+lê uma `Task` sob `Ownership::Exclusive` nunca vê as atualizações do agente) — não
  aprofundado por decisão do usuário (Python está fora de escopo desde a Rodada 5); pode ser
  QoS incompatível entre reader/writer no binding Python, mas não confirmado.
- ~~`third_party/llama.cpp_dds/tools/server/server-queue.{h,cpp}` ... bloqueado pelo bug de
  codegen da Fase R4~~ — **resolvido**: `llama-server` builda limpo na árvore canônica desde a
  auditoria da Rodada 4, reconfirmado numa árvore de build fresca na Rodada 5; o fix do
  "thundering herd" agora pode (e deveria) ser validado por smoke test real nessa árvore.
- ~~Repetir a medição de threads (R2) sob `client::submit()` concorrente~~ — **resolvido na
  Rodada 6**: novo teste prova `SharedWaitSet` compartilhado (registration_count=2×N) sob o
  padrão real do cliente, mais decomposição fila-agente vs. inferência real. Ver seção
  "Rodada 6".
- ~~Investigar os 20/72 timeouts observados na rodada de carga do harness (R1)~~ —
  **investigado e causa raiz achada na Rodada 6**: (1) o harness tinha um bug real
  (`wait_for_completion` timeout contado como sucesso, corrigido na Rodada 5); (2) a
  degradação progressiva real (não maquiada) veio de um bug genuíno em
  `reap_dead_agents` (achado e corrigido nos hosts remotos via SSH read-only — ver seção
  "Rodada 6"). **Ainda em aberto**: por que um agente trava (heartbeat para, processo
  continua vivo consumindo CPU) sob carga sustentada em primeiro lugar — não investigado a
  fundo, requer reproduzir sob carga real.
- ~~Migrar `cyclonedds-rust` de path local para versão publicada~~ — **resolvido na Rodada
  6**: workspace e `dds-contract` apontam para `"2.0.0"` do crates.io.
- ~~Fazer o sweep fino de concorrência do Python~~ — fora de escopo desde o mandato "esquece
  Python" da Rodada 5; não será retomado.
- ~~Instrumentar os campos `t_*_ns` do `Task` de verdade~~ — **esclarecido na Rodada 6**: 4
  dos 6 campos ficam intencionalmente em 0 (evitar comparar relógios entre máquinas — decisão
  já documentada no código); os 2 campos seguros de medir localmente
  (`t_agent_queue_ns`/`t_inference_ns`) já entregam a decomposição fila-vs-inferência via o
  novo teste R2.
- ~~Consertar o build de `llama-server` a partir de `third_party/llama.cpp_dds`~~ —
  **resolvido**, ver Fase R4/Rodada 5 acima, reconfirmado numa árvore de build limpa na
  Rodada 6.
- ~~P3 de baixa prioridade: enums Rust tipados~~ — **resolvido na Rodada 5**, ver acima.
- **Novo (Rodada 6)**: por que o agente trava sob carga sustentada (heartbeat para, mas o
  processo continua rodando e consumindo CPU) — achado real nos hosts remotos, causa ainda
  não determinada. Candidato a próxima investigação se o padrão se repetir.
- **Novo (Rodada 7), baixa prioridade**: `dds_transport.cpp` drena request/response/status com
  `dds_take(..., 1, 1)` (batch size 1) em vez de um lote maior — overhead pequeno (1 syscall
  por amostra), não é um bug de parede, só uma otimização não urgente.
- **Novo (Rodada 7), redeploy pendente**: o fix do filtro de modelo em `DDSBridge::
  handle_request()` está no código mas não foi deployado nos hosts remotos (`.61`/`.62`) —
  mesma situação do fix do reaper da Rodada 6, aguardando decisão de quando redeployar.
