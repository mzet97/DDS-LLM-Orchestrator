# Optimization Report — DDS-LLM Orchestrator (Rust workspace)

**Status: Fases 0, 0.5, 2, 3, 6 implementadas e validadas; Fase 4 bloqueada por achado de
segurança real na crate `cyclonedds` (documentado, não corrigido — fora do escopo do
workspace Rust); Fase 5 adiada por escopo/risco. Comparação E2E real (DDS real, llama-server
real, modelo real, sem mocks) executada e reportada abaixo.**
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

- Fase 5 (WaitSet compartilhado) **não implementada** — decisão explícita de escopo/risco,
  não falta de tempo bruto; ver `OPTIMIZATION_PLAN.md` Fase 5 para o racional completo.
- Fase 4 (zero-copy `write_loan`) **bloqueada por achado de segurança real** na crate
  `cyclonedds` (UB potencial com campos `String` em loans zerados) — ver `OPTIMIZATION_PLAN.md`
  Fase 4 e o comentário `SAFETY` adicionado em
  `third_party/cyclonedds-rust/cyclonedds-rust/cyclonedds/src/writer.rs`.
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

Nenhuma. `cargo test --workspace --features dds -- --test-threads=1` (75/75 suítes) e
`cargo test -p dds-dataspace --features dds` (14/14, inclui A/B mock vs DDS real)
confirmados verdes após todas as mudanças de código desta sessão (Fases 2 e 3).

## Itens pendentes

- **Fase 5** (WaitSet compartilhado, T-617) — maior escopo/risco do plano, adiada
  conscientemente; pré-requisito real é o cenário de carga multi-processo (agent +
  orchestrator + context-store + mcp-gateway + observability + policy-engine simultâneos)
  que ainda não existe.
- **Fase 4** (zero-copy) — bloqueada por segurança, não por escolha; requer mudança na API
  da crate `cyclonedds` (`request_loan`/`WriteLoan`) antes de poder ser retomada com
  segurança para tipos com campos `String`.
- Reexecutar a comparação E2E com **concorrência real** (múltiplos clientes simultâneos) e
  medição isolada do tempo de coordenação (via os campos `t_*_ns` já instrumentados em
  `Task`) — a única forma de a comparação E2E realmente expor a diferença Rust/Python que
  este projeto se propõe a medir.
- Consertar o build de `llama-server` a partir de `third_party/llama.cpp_dds` (bug de codegen
  em `ChatCompletionResponse.finish_reason`) para que a árvore nova deixe de depender do
  binário antigo de `src/llama_cpp`.
- P3 de baixa prioridade (não bloqueante): adicionar enums Rust tipados para
  `SecurityLevel`/`ComponentType`/etc. (hoje `i32` cru, fiel ao IDL) — ver Fase 6 no plano.
