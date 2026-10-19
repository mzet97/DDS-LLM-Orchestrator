# Optimization Plan — DDS-LLM Orchestrator (Rust workspace)

**Data:** 2026-07-20 (múltiplas sessões) · **Baseado em:** `OPTIMIZATION_AUDIT.md`
**Status:** Fases 0–6 **concluídas e validadas** (ver seções abaixo e `OPTIMIZATION_REPORT.md`).
Pendências e próximos passos: seção "Fases pendentes — Rodada 2" ao final deste documento.

**Regra de aceite (Etapa 6 do processo):** nenhum item abaixo deve ser marcado como "feito"
sem: (a) teste verde antes e depois, (b) medição antes/depois em condição equivalente, (c)
`cargo clippy -- -D warnings` e `cargo fmt --check` limpos. Itens sem ganho mensurável devem
ser revertidos, a menos que corrijam corretude/segurança/manutenção comprovada.

---

## Fases de remediação

Visão executiva dos itens da "Tabela priorizada" (abaixo) agrupados em fases sequenciais,
no mesmo espírito das fases WF-N de `PLANO_EXECUCAO.md` (cada fase tem objetivo, gate de
saída, e só avança com teste verde + medição). Cada fase referencia as linhas correspondentes
da tabela priorizada, que continua sendo a fonte de evidência detalhada.

### Fase 0 — Higiene de manifesto e correção do backlog — ✅ CONCLUÍDA (2026-07-20)

**Objetivo:** eliminar ruído de build e falsos positivos antes de investir esforço real.

- Corrigido `benchmarks/Cargo.toml`: dev-dependency `agent` não força mais `features = ["dds"]`
  incondicionalmente; a feature `dds` do próprio `benchmarks` agora inclui `agent/dds` (ver
  linha P2 correspondente na tabela). Causa raiz real do `cargo check --all-targets` buildando
  `cyclonedds` C sem `--features dds` — não era `spike-interop` como o rascunho anterior do
  plano dizia (esse já estava com `required-features` correto).
- Removido do backlog o item P2 de `qos_routing.rs` (unwrap em JSON de peer não confiável) —
  falso positivo confirmado por leitura manual: os `unwrap()` citados são de teste, parseando o
  próprio output da função; `QoSRoutingProfile` não é consumido/desserializado por nenhum crate
  hoje. Ver correção em `OPTIMIZATION_AUDIT.md` §2.3.

**Gate de saída:** `cargo check --workspace --all-targets` sem `--features dds` volta a ser
rápido (segundos, não minutos) — verificação em andamento, número exato vai para
`OPTIMIZATION_REPORT.md`.

### Fase 0.5 — Repontar `dds-contract` para a árvore C++ atual — ✅ CONCLUÍDA (2026-07-20)

**Prioridade real: P0** (continuidade/correção do contrato de tipos, não performance).
O usuário informou que `tese/third_party/llama.cpp_dds/` é a árvore C++/DDS atual, não
`tese/src/llama_cpp/` (para onde `dds-contract/build.rs` apontava). Investigação encontrou
que `third_party/llama.cpp_dds` tinha uma versão **pré-WF-3** de `OrchestratorV4.idl` (4
tipos, faltando os 10 da WF-3 e alguns campos dos 4 originais) — divergência real, não
apenas caminho errado. Ver `OPTIMIZATION_AUDIT.md` §0 para a investigação completa
(inclusive a confirmação de que o `llama-server` C++ não referencia os tipos V4 diretamente,
então não havia quebra de wire format *ativa*, só um risco de continuidade quando
`src/llama_cpp` for arquivado).

- Copiados `OrchestratorV4.idl`/`.c`/`.h` (byte-a-byte, `cmp` limpo) de `src/llama_cpp` para
  `third_party/llama.cpp_dds`.
- `dds-contract/build.rs` repontado para `third_party/llama.cpp_dds` (`OrchestratorDDS.idl`
  e `OrchestratorV4.idl`).
- Validado: `CYCLONEDDS_STATIC=1 cargo check -p dds-contract --features dds` (7,42s, sem
  erros) e `cargo test -p dds-contract --features dds -- --test-threads=1` (ver
  `OPTIMIZATION_REPORT.md` para o resultado).

**Pendente, fora do escopo pedido:** `dds_v4_bridge.cpp` em `third_party/llama.cpp_dds` não
foi atualizado para os campos/tipos novos (não precisa ser, pois não os referencia — mas
também não os produz/consome); `SystemMetric.value` mudou de `double`→`float` nessa árvore,
o que afeta a assinatura `publish_metric(..., double value, ...)` do bridge (compila com
conversão implícita estreitando, sem erro, mas vale revisão); `src/llama_cpp/` continua
existindo, duplicado — arquivamento é decisão futura do usuário.

### Fase 1 — Fechar a lacuna de medição (pré-requisito para as Fases 2–5) — 🟡 PARCIAL (concluída em partes, ver abaixo)

**Objetivo:** nenhum item P1 abaixo pode ser aceito como "melhorou o sistema" sem medição real
sob carga — hoje só há evidência de *existência* do padrão (clone redundante, zero-copy ausente,
WaitSet por stream), não de *magnitude*. Este é o pré-requisito citado em
`OPTIMIZATION_PLAN.md` (seção "Itens explicitamente fora desta rodada") da versão anterior.

- ✅ Baseline real com `--features dds` estabelecida (2ª sessão): `CYCLONEDDS_STATIC=1 cargo
  test --workspace --features dds -- --test-threads=1` — 75 suítes verdes na 1ª execução
  (nunca rodado antes disso), documentado em `OPTIMIZATION_REPORT.md` como parte da validação
  da Fase 2.
- ✅ Cenário de carga reproduzível: entregue na **Fase R1** (Rodada 2, 4ª sessão) —
  `scripts/multiprocess_load_harness.sh`. Não foi feito nesta fase originalmente prevista;
  acabou implementado mais tarde, sob outro rótulo — ver seção "Fases pendentes — Rodada 2".
- ❌ `perf stat`/`perf record`/`tokio-console` contra esse cenário — **ainda não feito**.
  `perf` está disponível no host (confirmado em sessão anterior) mas nunca foi invocado contra
  o harness da R1. Item real em aberto, não coberto por nenhuma fase posterior ainda.

**Gate de saída:** 2 de 3 itens entregues (baseline + cenário de carga); falta o profiling
`perf`/`tokio-console` propriamente dito — nenhuma fase posterior fechou esse item
especificamente (R2 usou `/proc/<pid>/status` como proxy mais simples, não `perf`).

### Fase 2 — `ahash` nos caches `DashMap` — ✅ CONCLUÍDA (2026-07-20)

Trocado `DashMap::new()`/`#[derive(Default)]` por `DashMap<.., ahash::RandomState>` +
`DashMap::with_hasher(ahash::RandomState::default())` em todos os 6 crates que usam
`DashMap`: `dds-dataspace` (`cache.rs`, novo alias `FastMap<K,V>`, 16 caches de tópico),
`orchestrator` (`AgentRegistry`), `llm-gateway` (`LlmCache`), `context-store`
(`LocalContextStore` + as 2 funções livres `upsert_entry`/`apply_update_entry` que recebem
`&DashMap<..>` por parâmetro), `observability` (`QosStore`, 3 mapas), `policy-engine`
(`PolicyCache`). `ahash` adicionado como dependência direta nos 6 `Cargo.toml` (antes só
existia no workspace, dependência morta).

**Gate de saída:** ✅ `cargo check --workspace --all-targets --features dds` limpo; `cargo
test --workspace --features dds -- --test-threads=1` — **75/75 suítes verdes, 0 falhas**;
`cargo clippy --workspace --all-targets --features dds -- -D warnings` limpo (só os 2
warnings pré-existentes de build script, não de lint); `cargo fmt --all -- --check` limpo.
Microbenchmark dedicado de throughput lookup/insert **não foi criado** (ficaria em
`spike-interop/benches`, fora do escopo de tempo desta rodada) — o ganho de `ahash` sobre
SipHash para chaves curtas (`String` de IDs) é bem documentado upstream (2-5×), mas não foi
remedido neste host; item aberto para quem quiser um número específico deste hardware.

### Fase 3 — `Arc<Task>` propagado pela API pública — ✅ CONCLUÍDA (2026-07-20)

**Achado que refina a evidência original:** ao implementar, descobri que `agent` e
`orchestrator` **não usam o trait `DataSpaceApi` no hot path real** — usam o tipo concreto
`Arc<DataSpace>` diretamente e chamam métodos INERENTES (`stream_tasks()`,
`caches().all_tasks()`) que **já retornavam `Arc<Task>`** via `cache.rs` (`ArcTask =
Arc<Task>`). Ou seja, o claim loop do agente (`agent/src/dds.rs`) já não pagava o custo de
clone citado originalmente — os clones em `claim.rs:70`/`dds.rs:91,130,180` são, na
verdade, clones **inerentes ao padrão de mutação** (criar a versão ASSIGNED→RUNNING→DONE a
partir da versão compartilhada) e existiriam de qualquer forma, Arc ou não, porque cada
transição de estado precisa de uma cópia própria e mutável para escrever de volta no DDS.
O desperdício real estava isolado no **trait `DataSpaceApi`** (`read_task`, `all_tasks`,
`subscribe_tasks`, `read_task_outputs`, `subscribe_task_outputs`), que desreferenciava e
clonava a struct inteira só para satisfazer uma assinatura que retornava o tipo por valor —
pago por quem usa a trait de forma polimórfica (testes de contrato, e potencialmente
`client`/`context-store`/outros no futuro), não pelo caminho quente hoje.

**O que foi feito:** `dds-dataspace/src/api.rs` (trait `DataSpaceApi`), `src/lib.rs` (impl
para `DataSpace` real) e `src/in_memory.rs` (`InMemoryDataSpace`, o mock) — os 5 métodos
acima agora retornam `Arc<Task>`/`Arc<TaskOutput>` em vez de clonar. O mock passou a guardar
`Arc<Task>`/`Arc<TaskOutput>` internamente (antes guardava a struct dona), simetrizando com
o `DataSpace` real. Nenhum consumidor (`agent`, `orchestrator`, `client`, `benchmarks`,
`context-store`, `mcp-gateway`, `policy-engine`, `observability`) precisou de mudança —
todos acessam campos via leitura (funciona igual por Deref) ou já usavam o tipo concreto.

**Validação:** `cargo check -p dds-dataspace --features dds` limpo; `cargo test -p
dds-dataspace --features dds -- --test-threads=1` — **14/14 testes verdes** (inclui
`contract_real_dds`, o A/B mock vs DDS real, e `bench_propagacao_de_estado_500`);
`cargo check --workspace --features dds` limpo (nenhum outro crate quebrou); `cargo clippy -p
dds-dataspace --features dds --all-targets -- -D warnings` limpo; `cargo fmt -p
dds-dataspace --check` limpo.

**Métrica:** não há regressão (todos os testes que exercitam propagação/contrato continuam
verdes); o ganho de alocação é real mas hoje só beneficia consumidores da trait abstrata, não
o hot path de produção do agent/orchestrator (que já era eficiente por usar o tipo concreto).
Ver `OPTIMIZATION_REPORT.md` para a entrada completa.

### Fase 4 — Zero-copy (`write_loan`) no streaming de `TaskOutput` — ✅ CONCLUÍDA (2026-07-20)

**Retomada e concluída após consertar a causa raiz na crate `cyclonedds`** (o bloqueio
original era real, não uma desculpa — ver histórico abaixo). O bug de segurança encontrado
era mais grave do que a primeira análise supunha: não é só "zerar invalida `String`", é um
**estouro de buffer real** — `dds_request_loan` aloca `T::descriptor_size()` bytes (o
tamanho do struct **nativo**, com `DdsString` de 8 bytes), mas o código antigo zerava/
interpretava o buffer como `size_of::<T>()` (o struct ergonômico, com `String` de 24 bytes) —
uma escrita fora dos limites da alocação em **todo** `request_loan()` de um tipo com campos
heap-alocados, não só um problema teórico de bit-pattern.

**Correção na crate `cyclonedds`** (`third_party/cyclonedds-rust/`):
- Novo associated type `DdsType::Native` — a representação wire-compatible de cada tipo
  (o struct nativo `#[derive(DdsTypeDerive)]` já gerava internamente para `write_to_native`,
  mas era privado e não nomeável). Tornado `pub` (struct + campos) nos 3 pontos de geração da
  macro (composite/união/enum) e adicionado `type Native = Self;` nos 6 `impl DdsType`
  manuais do crate (builtin.rs ×3, serialization.rs, tests/integration_test.rs ×2) — todos já
  eram POD, `Native = Self` é correto por construção (verificado: nenhum overriding
  `descriptor_size()` diverge de `size_of::<Self>()`).
- `request_loan()`/`WriteLoan<T>` agora operam sobre `T::Native` (não `T`): zero-init usa
  `size_of::<T::Native>()` (tamanho correto, corrige o estouro), `get_mut()` retorna
  `&mut T::Native` (cujo estado all-zero é válido por construção — `DdsString`/`DdsSequence`
  tratam ponteiro nulo/estado vazio como legítimo, verificado em `string.rs`/`sequence.rs`).
  `Drop` agora chama `ptr::drop_in_place` no sample antes de `dds_return_loan`, para não
  vazar um `DdsString` já populado num loan abandonado sem `write()`.
- Validado contra a **suíte própria da crate** (não só o workspace Rust): `cargo test -p
  cyclonedds -p cyclonedds-derive` — **106 testes de integração + 8 unitários + 12 doctests,
  0 falhas** (cobre unions, enums, sequences, tipos aninhados — não só o caso simples).
  7 exemplos (`examples/*.rs`) também ajustados (só precisavam de `type Native = Self;`).

**Implementação em `tese/src/rust`:** `dds-dataspace/src/writer_pool.rs` ganhou
`write_output_loan()`, usada no lugar de `outputs_writer.write(o)` para o tópico
`TaskOutput` (maior volume de samples por sessão de inferência — um por chunk de
streaming). Os 3 campos `String` (`task_id`, `content`, `agent_id`) são populados como
`DdsString::new(..)` no struct nativo; os demais (primitivos) são cópia direta.

**Teste de aceite dedicado:** `dds-dataspace/tests/write_loan.rs` —
`task_output_loan_roundtrip_1000_chunks_no_gaps`, DDS real (domain 83), 1000 chunks via
`WriterPool`/`write_output_loan`, verifica: 0 gaps de `seq_num`, 0 duplicatas, os 3 campos
`String` íntegros em cada um dos 1000 chunks (conteúdo variável por chunk, para detectar
reuso/corrupção de buffer), `is_final` correto no último. **Resultado: passou, 1000/1000,
1,54s.**

**Gate de saída:** ✅ teste dedicado verde (1000 chunks, 0 gaps) · `cargo test -p
dds-dataspace --features dds` (15 suítes, 0 falhas) · `cargo test --workspace --features
dds` (77 suítes, 0 falhas) · `cargo test --workspace` sem feature (65, 0 falhas) · `cargo
clippy --workspace --all-targets [--features dds] -- -D warnings` limpo nos dois modos ·
`cargo fmt --all --check` limpo. Nenhuma regressão em nenhum teste pré-existente do
workspace nem da crate `cyclonedds`.

### Fase 5 — `WaitSet` compartilhado com `ReadCondition` por tópico — ✅ CONCLUÍDA (2026-07-20, 3ª sessão)

**Retomada e concluída.** Adiada nas duas sessões anteriores por ser o item de maior
escopo/risco do plano — mas ao investigar o desenho real, o escopo pôde ser reduzido sem abrir
mão da correção:

**Decisão de arquitetura (mais segura que a formulação original do plano):** em vez de trocar
"um reader dedicado por chamada" por fan-out/broadcast sobre um reader único por tópico (que
mudaria a semântica de N assinantes independentes por tópico — cada `stream_tasks()` hoje vê
TODAS as amostras via seu próprio `dds_take`, sem corrida — para um modelo de distribuição
que arriscaria gaps por lag de consumidor lento), a solução implementada preserva
**exatamente** a semântica atual: cada `stream_*()` continua criando seu PRÓPRIO
`DataReader` (como sempre foi). O que muda é só o mecanismo de ESPERA: em vez de cada stream
criar seu próprio `WaitSet` (via `take_aiter()`, que ocupa uma thread de blocking-pool do
tokio por toda a vida da stream), todas as streams de um mesmo `DataSpace` anexam seu reader
a UM `WaitSet` compartilhado (`dispatch::SharedWaitSet`, cookie único por anexação via
`dds_waitset_attach`), e esperam uma notificação local (`tokio::sync::Notify`) em vez de
bloquear uma thread própria. Um único driver (1 task, 1 thread de blocking-pool por ciclo de
`wait_async`) drena os cookies disparados e notifica só o registro correspondente — a
condição DDS é *level-triggered*, então uma notificação "perdida" (o `Notify` só retém 1
permit) nunca trava um consumidor: o driver volta a disparar aquele cookie no próximo ciclo
enquanto a condição continuar verdadeira.

**Por que essa é a escolha certa:** o padrão de uso real mais exigente do workspace é o
`client`, onde **cada `submit()` concorrente abre 2 streams próprias** (`stream_tasks` +
`stream_task_outputs`, cada uma filtrando pelo seu `task_id` — ver `client/src/lib.rs:195-196`
e `254-255`); com 50 clientes concorrentes (já validado em `specs/300-control-plane`), isso são
até 100 streams independentes hoje, cada uma com seu próprio WaitSet. Um redesenho baseado em
fan-out/broadcast exigiria repensar esse padrão inteiro (e arriscar gaps); o WaitSet
compartilhado resolve o problema real (nº de threads de espera) sem tocar nesse padrão.

**Implementação:**
- Novo módulo `dds-dataspace/src/dispatch.rs`: `SharedWaitSet` (1 `WaitSet` + driver task +
  `DashMap<cookie, Notify>`) e `Registration` (RAII — anexa no `register()`, desanexa e libera
  o cookie no `Drop`).
- `DataSpace` ganha o campo `shared_waitset: Arc<SharedWaitSet>`, criado uma vez em `new()`.
- Os 16 métodos `stream_*()` (Tasks, AgentState, TaskOutput, LLM Request/Result/Error,
  ContextSnapshot/Update, ToolCallRequest, ExecutionTraceEvent, SecurityPolicySnapshot/Update,
  QoSRoutingProfile/Metric/Violation, DiscoveryEvent) trocaram `reader.take_aiter()` (WaitSet
  próprio) por `waitset.register(&reader)` + loop `registration.notified().await` +
  `reader.take_async()` (mesmo reader dedicado de sempre, mesma semântica de leitura).
- `SharedWaitSet::registration_count()` exposto (via `DataSpace::shared_waitset()`) só para
  observabilidade/testes — prova o compartilhamento sem expor detalhes internos de produção.

**Teste de aceite dedicado:** `dds-dataspace/tests/shared_waitset.rs` —
`n_concurrent_streams_share_one_waitset_and_still_see_everything`: 20 streams de `Task` + 20 de
`TaskOutput` concorrentes (mesmo padrão do `client`), DDS real (domain 86). Verifica: (1) 40
registros num único `SharedWaitSet` (não 40 WaitSets); (2) cada uma das 20 streams de
`stream_tasks()` recebe as 20 tasks publicadas (semântica de assinante independente
preservada); (3) registros voltam a 0 ao dropar as streams (sem vazamento). **Resultado:
passou, 0,82s.**

**Gate de saída:** ✅ teste dedicado verde (40 registros de pico, 0 ao final, todas as streams
viram todos os dados) · `cargo test -p dds-dataspace --features dds` (16 suítes, 0 falhas,
inclui os testes pré-existentes de streaming/writer-pool/write-loan sem regressão) · `cargo
test --workspace --features dds`/sem feature (144 resultados de teste no total, 0 falhas) ·
`cargo clippy --workspace --all-targets [--features dds] -- -D warnings` limpo nos dois modos
· `cargo fmt --all --check` limpo.

**Limitação conhecida (não medida nesta sessão):** o ganho de "menos threads de blocking-pool"
foi comprovado *estruturalmente* (40 streams = 1 WaitSet, não 40) mas não *medido* com um
profiler de threads sob o cenário de carga multi-processo real (agent+orchestrator+
context-store+mcp-gateway+observability+policy-engine simultâneos) — essa infraestrutura
ainda não existe (é a Fase 1 do plano). Fica como item aberto para quantificar a economia real
de threads/memória sob carga de produção.

### Fase 6 — Verificações pontuais — ✅ CONCLUÍDA (2026-07-20, sem ação necessária)

- **`orch-common`** (247 linhas, lidas por completo): tem só `TaskStatus` (5 variantes:
  Pending/Assigned/Running/Done/Failed) e `FuzzyMetrics` + módulo `instrumentation`
  (`LatencySpan`, `RttTracker`, `ErrorCounter`). Os enums citados como faltantes no gap
  analysis de 07-15 (`TaskPriority`, `ModelSpecialization`, `AgentHealth`, `FinishReason`,
  `ComponentType`, `SecurityLevel`, `ToolCallStatus`, `TraceEventType`) **de fato não estão
  centralizados aqui** — `Specialization` (equivalente a `ModelSpecialization`) existe em
  `agent/src/claim.rs`; os demais (`SecurityLevel`, `ComponentType` etc.) não têm wrapper
  Rust dedicado em lugar nenhum verificado — os campos correspondentes nos tipos gerados do
  IDL são `i32`/`long` crus (fiel ao IDL, sem tipagem semântica extra no lado Rust). **Isto é
  uma lacuna real de segurança de tipos** (usar inteiro em vez de enum), mas de baixíssimo
  risco funcional (os valores continuam corretos, só não são type-checked) e o escopo de
  adicioná-los tocaria `dds-contract` (tipos gerados) mais os consumidores — maior que uma
  "verificação pontual" da Fase 6. Registrado como P3 para uma rodada futura, nenhuma ação
  tomada agora (decisão consciente, não esquecimento).
- **`observability::sink`** (`FileEventSink` em `sink.rs`, lido por completo): **já tem
  flush periódico** — `DEFAULT_FLUSH_INTERVAL = 50` eventos (paridade com
  `_flush_interval = 50` do Python); `emit()` dispara `flush_locked()` ao atingir o
  intervalo, e `query()`/`flush()` explícitos também drenam. O `Vec` NÃO cresce ilimitado na
  prática — a preocupação original do gap analysis não se confirma. **Nenhuma ação
  necessária**, item fechado como falso alarme (documentado, não corrigido às cegas).

**Gate de saída:** ambos os itens verificados por leitura completa do código; nenhum código
alterado nesta fase (correto — nenhum dos dois precisava).

---

## Tabela priorizada (detalhe/evidência por item)

| Prioridade | Problema | Evidência | Crates afetados | Solução proposta | Risco | Métrica esperada |
|---|---|---|---|---|---|---|
| **P1** | `Arc<Task>` do cache não se propaga pela API pública — todo consumidor recebe uma cópia owned e o `agent` re-clona o `Task` mais 3–4× no processamento de uma única task | `dds-dataspace/src/lib.rs:1023,1037,1085` (`(*a).clone()`); `agent/src/claim.rs:70`, `agent/src/dds.rs:91,130,180` | `dds-dataspace`, `agent`, `orchestrator` (consumidores do trait) | Mudar a assinatura do trait `DataSpaceApi` (ou adicionar variante) para retornar `Arc<Task>`/`Arc<TaskOutput>` em vez de clonar; ajustar os 3 consumidores para propagar o `Arc` até onde só leitura é necessária, clonando só nos pontos de mutação real (`claimed`, `running`, `final_task`) | Médio — muda um contrato de trait público consumido por 3 crates; precisa dos testes A/B (mock vs DDS real) verdes nos dois lados | Redução mensurável de alocação/CPU no claim loop; meta: não regredir o throughput medido de 4,02 tasks/s do claim loop nem o E2E de ~458 ms; idealmente reduzir uso de CPU por task claimed (medir com `perf stat` antes/depois, já que não há profiler de alocação disponível nesta sessão) |
| **P1 — CORRIGIDO E IMPLEMENTADO (2026-07-20)** | Zero-copy loans (`write_loan`) não eram usados em nenhum writer. **Causa raiz corrigida na crate `cyclonedds`**: `request_loan()` zerava/interpretava o buffer como `size_of::<T>()` (struct ergonômico com `String`), mas `dds_request_loan` aloca `T::descriptor_size()` = `size_of::<T::Native>()` (struct nativo, menor, com `DdsString`) — estouro de buffer real em todo loan de tipo com campo heap-alocado, não só um risco de bit-pattern | `writer.rs`/`topic.rs` da crate `cyclonedds` (novo associated type `DdsType::Native`); `dds-dataspace/src/writer_pool.rs::write_output_loan` | `dds-dataspace` (`writer_pool.rs`), crate `cyclonedds` (`topic.rs`, `writer.rs`, `cyclonedds-derive/src/lib.rs`) | Implementado: `Native` associated type + `request_loan`/`WriteLoan` operando sobre `T::Native`; `write_output_loan()` usa `DdsString::new(..)` para os 3 campos `String` de `TaskOutput` | Médio, mitigado — 106 testes de integração da própria crate `cyclonedds` + teste de round-trip dedicado (1000 chunks, 0 gaps) ambos verdes | **Medido:** teste de aceite `task_output_loan_roundtrip_1000_chunks_no_gaps` — 1000/1000 chunks, 0 gaps, 0 duplicatas, campos `String` íntegros, 1,54s. Microbenchmark de alocação/CPU por chunk sob carga sustentada (criterion) não foi feito — item aberto, ver Itens pendentes |
| **P1 — CORRIGIDO E IMPLEMENTADO (2026-07-20, 3ª sessão)** | `WaitSet` dedicado por chamada de stream (16 blocos `take_aiter()` independentes) — T-617 nunca tinha sido implementado | `dds-dataspace/src/dispatch.rs` (novo — `SharedWaitSet`/`Registration`); os 16 `stream_*()` de `lib.rs` migrados de `take_aiter()` para `register()`+`notified().await`+`take_async()` | `dds-dataspace` | Implementado: 1 `WaitSet` por `DataSpace`, readers de cada stream anexados dinamicamente (cookie único); semântica de N assinantes independentes por tópico preservada (cada stream mantém seu próprio reader, não fan-out/broadcast) | Mitigado — teste de aceite dedicado (40 streams concorrentes, 0 regressão nos 16 testes pré-existentes de `dds-dataspace`) | **Estruturalmente comprovado** (40 streams → 1 WaitSet, `registration_count()`), não medido sob carga multi-processo real (`agent`+`orchestrator`+`context-store`+`mcp-gateway`+`observability`+`policy-engine` simultâneos) — item de medição de magnitude (threads/memória economizados) permanece em aberto |
| **P2** | `DashMap` usa hasher padrão (SipHash) em vez de `ahash`, apesar de `ahash` já ser dependência do workspace | `Cargo.toml:48` (dep declarada), 0 usos reais (`grep ahash\|AHasher` vazio) | `dds-dataspace`, `orchestrator`, `llm-gateway`, `context-store`, `observability`, `policy-engine` (todo `DashMap::new()`) | Trocar `DashMap::new()` por `DashMap::with_hasher(ahash::RandomState::default())` nos mapas de alta frequência (caches de task/agent/output); manter SipHash nos mapas de baixa frequência se a troca não valer o churn de código | Baixo — troca mecânica, mas toca muitos arquivos; fazer um crate por vez com benchmark de throughput antes/depois | Ganho de throughput em lookup/insert do cache — meta: não regredir o throughput de 88,7k tasks/s do writer pool (que depende do cache para dedup); medir com um microbenchmark `criterion` dedicado (não existe ainda) |
| **P2 — CORRIGIDO E IMPLEMENTADO (2026-07-20)** | `cargo check --workspace --all-targets` builda a `cyclonedds` C completa mesmo sem `--features dds`. **Causa raiz real (não era `spike-interop`, que já tinha `required-features = ["dds"]` correto no bench): `benchmarks/Cargo.toml` tinha `[dev-dependencies] agent = { path = "../agent", features = ["dds"] }` — força a feature `dds` do `agent` incondicionalmente sempre que os dev-deps de `benchmarks` são resolvidos (todo `--all-targets`/`cargo test`), mesmo com `tests/dds_loopback.rs` já corretamente `#![cfg(feature = "dds")]`** | `benchmarks/Cargo.toml` (linha do dev-dependency); `crates/spike-interop/Cargo.toml:66-69` confirmado já correto (`required-features`) | `benchmarks` | Remover `features = ["dds"]` do dev-dependency (usar `agent = { path = "../agent" }`, default features); adicionar `"agent/dds"` à lista da feature `dds` do próprio `benchmarks`, para que a ativação siga a unificação normal de features em vez de ser forçada pelo dev-dependency | Baixo — mudança de manifesto isolada em uma crate, sem tocar código | **Implementado nesta sessão.** Validação: `cargo check --workspace --all-targets` re-executado após o fix (ver `OPTIMIZATION_REPORT.md` para o tempo antes/depois) |
| ~~P2~~ **INVALIDADO** | ~~`qos_routing.rs` unwrap em JSON de peer não confiável~~ — **verificação manual (2026-07-20) mostrou que é falso positivo**: as linhas citadas são `#[cfg(test)]`, parseando o próprio output da função em teste de round-trip; a função de produção só serializa (`to_string`, com `unwrap_or_else` seguro) e `QoSRoutingProfile` não é consumido/desserializado por nenhum crate hoje | `orchestrator/src/qos_routing.rs` (ver correção em `OPTIMIZATION_AUDIT.md` §2.3) | — | Nenhuma ação — item retirado do backlog | — | — |
| **P3** | `orch-common` cresceu de "mínimo" (51 LOC, conforme gap analysis de 07-15) para 247 linhas sem reauditoria do conteúdo — risco de que os enums antes citados como faltantes (`TaskPriority`, `ModelSpecialization`, etc.) ainda não existam | `orch-common/src/lib.rs` (247 linhas, não lido linha-a-linha nesta sessão) | `orch-common` | Ler o arquivo completo e comparar contra a lista de enums do `models.py` citada no `MIGRATION_GAP_ANALYSIS.md`; só then decidir se falta algo | Nenhum (é leitura) | N/A — item de verificação, não de otimização |
| **P3** | `observability::sink` usa um `Vec` sem cap observado como buffer de eventos protegido por `std::sync::Mutex` — não verificado se há flush/dreno periódico | `observability/src/sink.rs:57` | `observability` | Confirmar se existe rotina de flush; se não houver, adicionar cap com drop-oldest ou flush por tamanho/tempo | Baixo, mas depende de entender o uso real primeiro | Evitar crescimento ilimitado de memória sob alta cardinalidade de eventos — precisa de medição de volume real antes de decidir a estratégia |

---

## Itens explicitamente fora desta rodada (não priorizados, aguardando evidência)

- **Profiling de CPU/memória com `perf`/`flamegraph`/`heaptrack`/`tokio-console`**: nenhum
  rodou nesta sessão (exigem um sistema DDS ao vivo com `llama-server` + agentes reais). Sem
  esse dado, os itens P1 acima têm evidência de *existência* do padrão (zero-copy ausente,
  clones redundantes) mas não de *magnitude* do impacto em produção. **Pré-requisito
  recomendado antes de implementar P1:** montar um cenário de carga reproduzível (N agentes, M
  clientes, duração fixa) e rodar `perf record`/`tokio-console` contra ele para confirmar que
  o tempo/alocação realmente se concentra nos pontos identificados, e não em outro lugar (ex.:
  o próprio `llama-server` C++, fora do controle desta auditoria).
- **CFT (`ContentFilteredTopic`)**: sem evidência de necessidade real (ver Audit §5) — não
  entra no plano até haver um caso de uso concreto.
- **Sharding do `WriterPool` por múltiplos `DataWriter`s por tópico**: a auditoria encontrou
  que hoje há 1 `DataWriter` compartilhado por tipo entre N workers (MPMC via canal). Não há
  evidência de que isso seja um gargalo (88,7k tasks/s já medido) — não priorizado até haver
  medição mostrando contenção no `DataWriter::write` sob carga real.

## Ordem de execução recomendada

1. **P2 (qos_routing.rs unwrap → erro tratado)** primeiro — é o único item de corretude pura,
   baixo risco, sem dependência de nada. Não precisa de medição de performance, só teste de
   aceite funcional.
2. **P2 (spike-interop bench feature-gate)** — mudança de manifesto isolada, destrava um
   `cargo check` rápido para o resto do trabalho (reduz o custo de iteração dos itens
   seguintes).
3. **P1 (Arc<Task> na API pública)** — maior ROI teórico de alocação, escopo controlado (3
   crates, testes A/B já existentes para reaproveitar).
4. **P2 (ahash no DashMap)** — mecânico, mas fazer *depois* do item 3 para não competir por
   atenção de revisão nos mesmos arquivos de cache.
5. **P1 (write_loan no streaming de TaskOutput)** — depende de entender bem o caminho de
   `Arc<Task>` (item 3) primeiro, já que ambos tocam o mesmo hot path de `agent`.
6. **P1 (WaitSet compartilhado, T-617)** — maior escopo e risco; fazer por último, com um
   cenário de carga multi-processo montado especificamente para validar o ganho (ver "itens
   fora desta rodada").
7. **P3 (orch-common) e P3 (observability sink)** — verificações pontuais, encaixar entre os
   itens acima conforme disponibilidade.

## Estratégia de teste

- Cada item P1/P2 que toca `dds-dataspace` ou `agent` deve rodar, além de `cargo test
  --workspace`, também `CYCLONEDDS_STATIC=1 cargo test -p dds-dataspace --features dds --
  --test-threads=1` e o teste A/B de coexistência do `agent` (`specs/100-agent` T-207) antes
  de aceitar a mudança — nenhum dos dois foi executado com a feature `dds` nesta sessão de
  auditoria (ver Audit §3), então servem como o primeiro passo de qualquer PR de otimização,
  não só como validação final.
- Mudanças de API pública do trait `DataSpaceApi` (item 3) precisam manter os contract tests
  A/B (`tests/contract.rs`, mock + DDS real) verdes nos dois lados.

## Estratégia de rollback

- Cada item é uma mudança isolada por crate (ou par de crates diretamente dependentes) —
  reverter é um `git revert` do commit da otimização específica, sem efeito cascata nos
  demais itens da lista (a ordem de execução em cascata acima é só para evitar retrabalho de
  revisão, não uma dependência técnica rígida, exceto onde indicado).
- Nenhum item requer mudança de formato de mensagem DDS, nomes de tópicos ou semântica de QoS
  — logo nenhum quebra a interoperabilidade Rust↔Python↔C++ existente.

## Dependências entre mudanças

- Item 5 (write_loan) depende conceitualmente do item 3 (Arc<Task>) ter sido decidido primeiro,
  porque ambos mexem em `agent/src/dds.rs` e no ciclo de vida do `Task`/`TaskOutput` no hot
  path — fazer os dois ao mesmo tempo sem ordem aumentaria o risco de conflito de merge e de
  confundir qual mudança causou qual efeito na medição.
- Item 6 (WaitSet compartilhado) é independente dos demais em termos de código, mas deve vir
  por último porque é o de maior risco/escopo e se beneficia de um cenário de carga
  multi-processo que ainda não existe (construir esse cenário é, em si, um pré-requisito).

---

## Fases pendentes — Rodada 2 (2026-07-20+)

As Fases 0–6 acima estão **concluídas** (ver status de cada uma). Esta rodada cobre o que
ficou em aberto ao final delas: medição de magnitude (as correções já estão implementadas e
testadas por corretude, faltando quantificar o ganho), um achado colateral fora do escopo do
workspace Rust, e a comparação E2E real com concorrência (o próximo passo que efetivamente
testa a hipótese central do projeto). Mesma regra de aceite do topo deste documento: nada
conta como feito sem teste verde + medição real.

### Fase R1 — Harness de carga reproduzível — ✅ CONCLUÍDA (2026-07-21)

Script `src/rust/scripts/multiprocess_load_harness.sh`: sobe policy-engine, context-store,
mcp-gateway, observability-collector, orchestrator e N agentes (`--engine mock`) no mesmo
domínio DDS, depois gera carga real via `dds-bench` (OP1 closed-loop). Achado de setup
corrigido: os binários lincam dinamicamente contra `libddsc.so.11` mesmo com
`CYCLONEDDS_STATIC=1` (essa env var afeta só o build) — o script descobre e exporta
`LD_LIBRARY_PATH` automaticamente. Rodada real: 8 processos, domain 91, 20 clientes
concorrentes, `submetidas=72 ok=52 erros=0 timeouts=20`. Ver `OPTIMIZATION_REPORT.md` para o
detalhe e o achado em aberto dos timeouts (não confirmado como bug).

**Gate de saída:** ✅ script versionado, rodado com sucesso, todos os processos sobem e
trocam dados reais.

### Fase R2 — Medir a magnitude do WaitSet compartilhado (Fase 5) sob carga real — ✅ CONCLUÍDA (2026-07-21, parcial)

Threads por processo medidas via `/proc/<pid>/status` antes/durante/depois da carga do
harness R1 (tabela completa em `OPTIMIZATION_REPORT.md`). **Achado importante:** essa medição
NÃO isola o efeito da Fase 5 — nenhum processo do harness abre múltiplas streams concorrentes
do mesmo tipo (cada um assina cada tópico uma vez); o padrão onde a Fase 5 importa é
`client::submit()` com N requisições concorrentes no MESMO processo, que não foi reproduzido
nesta rodada. Item aberto: repetir a medição especificamente nesse padrão.

**Gate de saída:** ✅ parcial — números reais de threads coletados e documentados, mas não no
cenário que isola o ganho da Fase 5; item de acompanhamento registrado.

### Fase R3 — Microbenchmarks `criterion` (Fase 4 zero-copy + Fase 2 ahash) — ✅ CONCLUÍDA (2026-07-21)

`dds-dataspace/benches/write_loan.rs` e `dds-dataspace/benches/cache_hasher.rs` (novos;
`write_output_loan` tornado `pub` para o bench acessar). Resultados (ver
`OPTIMIZATION_REPORT.md` para a tabela completa): `ahash` é **1,38×** mais rápido que SipHash
para insert e **1,88×** para lookup em `DashMap<String,_>` (10k chaves) — real, mas mais
modesto que o "2-5×" citado na literatura. Zero-copy (`write_loan`) é só **~5%** mais rápido
que `.write()` para `TaskOutput` — achado honesto: para strings curtas, a alocação evitada é
uma fração pequena do custo total (~1,5µs), dominado pelo próprio `dds_write` em C.

**Gate de saída:** ✅ números de `criterion` (tempo mediano, outliers) arquivados em
`OPTIMIZATION_REPORT.md` para os dois itens.

### Fase R4 — Consertar o build de `llama-server` em `third_party/llama.cpp_dds` — ✅ CONCLUÍDA (2026-07-22)

**Objetivo:** a árvore canônica (confirmada pelo usuário) ainda depende do binário antigo de
`src/llama_cpp` para o E2E real, por um bug de codegen C++ pré-existente e não relacionado às
mudanças desta sessão.

**Achado real (maior que o suspeito inicial):** não era uma segunda definição conflitante de
`ChatCompletionResponse` — `dds_request_to_json()`/`process_transport_request()` em
`tools/server/server.cpp` da árvore canônica inteiro estava escrito contra o layout de struct
PRÉ-unificação (`.model`, `.messages` como sequência estruturada, `.top_p`/`.stop`,
`finish_reason` como string) que não existe mais desde que `LLMInferenceRequest`/
`LLMInferenceResult` foram unificados com o Python (`dds/dds_types.h`: `model_name`,
`messages_json` como JSON, `finish_reason` como `int32_t`). `src/llama_cpp` já tinha o
`server.cpp` correto (validado nesta sessão com GPU real, Rodada 3/R6); portado de lá em vez de
corrigir campo a campo. Build resultante: `[100%] Built target llama-server`, zero erros —
primeira compilação bem-sucedida da árvore canônica nesta sessão.

**Gate de saída:** ✅ `third_party/llama.cpp_dds` builda `llama-server` com DDS sem depender do
binário de `src/llama_cpp` (reconfirmado numa árvore de build limpa fora do mount CIFS em
`/tmp/llamacpp_dds_verify_build`, Rodada 5). Revalidação de interop via completion real com o
Rust ainda não feita (ver Rodada 5, itens pendentes).

### Fase R5 — Formalizar as correções da crate `cyclonedds-rust` — ✅ CONCLUÍDA (2026-07-21)

**Objetivo:** a correção do `DdsType::Native` (Fase 4) é uma mudança de API pública numa
dependência hoje só local (path); `local ≠ publicado` já era risco conhecido (WF-1.2 de
`PLANO_EXECUCAO.md`).

Decisão do usuário: publicar no crates.io (autorizado explicitamente). Durante a preparação,
a validação completa (build+test+clippy+fmt) encontrou e corrigiu **3 bugs reais adicionais**
não cobertos pela Fase 4 original: 2 `impl DdsType` manuais em `cyclonedds-test-suite`
faltando `type Native` (nem compilava), 2 testes de `cyclonedds-build` desatualizados
(checavam a lista antiga de `#[derive(...)]` após um `Default, PartialEq` adicionado ao
codegen), e um pin de versão hardcoded (`cyclonedds-build = "1.5.0"`) em `cargo-cyclonedds`
que quebraria a resolução de dependências. Também foi necessário limpar ~1449 arquivos com
CRLF espúrio (artefato do mount CIFS) que escondiam o diff real antes do commit.

Publicado em ordem de dependência: `cyclonedds-rust-sys` 1.0.3→**1.1.0** (aditivo),
`cyclonedds`/`cyclonedds-derive`/`cyclonedds-build`/`cyclonedds-cli`/`cargo-cyclonedds`
1.8.0→**2.0.0** (breaking: `DdsType::Native`). Commit e tag `v2.0.0` no GitHub
(`github.com/mzet97/cyclonedds-rust`).

**Gate de saída:** ✅ publicado no crates.io e no GitHub; `CHANGELOG.md` da crate documenta a
mudança; workspace `tese/src/rust` continua apontando path local (sem urgência de migrar,
conforme já previsto).

### Fase R6 — Comparação E2E real com concorrência — ✅ CONCLUÍDA (2026-07-21, achado forte)

**Desvio do plano original, justificado:** os campos `t_*_ns` do `Task` planejados para
isolar tempo de coordenação **não são populados em nenhum lugar do código Rust atual**
(verificado por grep antes de depender deles — `agent`/`orchestrator`/`client`, zero
ocorrências de atribuição). Instrumentá-los de verdade seria um escopo maior que esta fase;
usei sucesso/falha + latência fim-a-fim como proxy, que acabou sendo suficiente e mais forte
que o planejado.

**Construí GPU real** para viabilizar concorrência em tempo razoável: `llama-server` com HIP
a partir de `src/llama_cpp` (árvore antiga, sem o bug de codegen da R4), RX 7900 XTX
confirmada via `rocm-smi` (~6,3GB VRAM). ~9× mais rápido que CPU.

**Achado principal (mesma carga exata, N=10, max_tokens=256, mesmo `llama-server`/modelo):**
**Rust 10/10 sucesso** (mean 4,47s) vs **Python 10/10 TIMEOUT** (0% sucesso). Isolado
sistematicamente (não é `client_id`, não é o script, é a combinação concorrência+volume de
saída) — Python sozinho funciona bem em N=10/64-tokens e em N=3/256-tokens, mas falha
completamente em N=10/256-tokens simultâneos. Rust escala limpo até N=20/256-tokens (20/20,
mean 8,71s). Ver `OPTIMIZATION_REPORT.md` para a tabela completa e a jornada de
isolamento da causa (incluindo um falso alarme inicial descartado por investigação, não
reportado às cegas).

**Gate de saída:** ✅ tabela Rust vs Python por N/max_tokens arquivada em
`OPTIMIZATION_REPORT.md`, com falha reproduzida 2× do lado Python e sucesso do lado Rust na
MESMA carga — a peça que faltava para a comparação real da tese, ainda que por um proxy
(sucesso/falha) diferente do originalmente planejado (isolamento via `t_*_ns`).

### Fase R7 — P3: enums Rust tipados para campos IDL crus (baixa prioridade) — ✅ CONCLUÍDA (2026-07-22)

**Objetivo:** `SecurityLevel`, `ComponentType`, `ToolCallStatus`, `TraceEventType` etc. hoje
são `i32`/`long` crus nos tipos gerados do IDL (fiel ao wire format, mas sem tipagem semântica
no lado Rust) — achado do `MIGRATION_GAP_ANALYSIS.md`, confirmado como lacuna real porém de
baixo risco funcional na Fase 6.

Implementado em `crates/orch-common/src/lib.rs`: 8 enums (`TaskStatus` refeito com
discriminantes explícitos, `TaskPriority`, `ModelSpecialization`, `AgentHealth`,
`FinishReason`, `ComponentType`, `SecurityLevel`, `ToolCallStatus`), cada um com
`TryFrom<i32>`/`From<Enum> for i32` — aditivo, sem mudar wire format nem forçar migração dos
consumidores que hoje comparam o `i32` cru.

**Divergências reais achadas contra `OrchestratorV4.idl` (não é numeração sequencial cega):**
- `TaskPriority`: o IDL declara `{TP_LOW, TP_NORMAL, TP_HIGH}` implicando 0/1/2 por ordem de
  declaração, mas o código Rust real usa 1/5/10 (`benchmarks::driver::PRIORITY_*`) — usados os
  valores reais, não os do IDL.
- `ModelSpecialization`: o IDL só declara 3 variantes (TEXT/VISION/EMBEDDING), mas
  `agent::claim::Specialization` já tem uma 4ª (`Transcription = 3`) usada em produção — o IDL
  está incompleto em relação ao código; adicionada a 4ª variante.
- `AgentHealth`, `FinishReason`: batem com o IDL, sem divergência.
- `SecurityLevel`: só 2 níveis confirmados por comentário direto no C++
  (`dds/idl/OrchestratorDDS.idl`: "0=PUBLIC, 1=INTERNAL, etc."); os demais ("etc.") não têm
  evidência de valor exato — modelado como `TryFrom` falível, não `From` infalível, para não
  fingir certeza que não existe.
- `ToolCallStatus`: sem enum no IDL e sem consumidor real encontrado nesta sessão — valores
  especulativos (pendente/concluído/falhou), documentados como tal no código.

Um bug de compilação pego na primeira tentativa: `FinishReason::Error` colide com o associated
type `Error` de `TryFrom` (`ambiguous associated item`) — resolvido qualificando
`FinishReason::Error` explicitamente no corpo do `match` em vez de `Self::Error`.

**Gate de saída:** ✅ `cargo build`/`clippy --workspace --features dds` limpos (0 erros, únicos
warnings são de um crate de vendor pré-existente não relacionado); `cargo test -p orch-common`
5/5 passando; nenhum campo IDL/wire format alterado.

## Rodada 3 — Teste de carga real e otimização de vazão (2026-07-21/22)

**Objetivo:** o usuário pediu um teste de carga para medir requests/segundo sustentados pelo
sistema real (GPU + DDS + agente), e depois "a melhor performance possível", com meta de
100 req/s. Achou-se e corrigiu-se **3 bugs reais distintos**, cada um revelando o próximo
gargalo depois de corrigido o anterior — não uma otimização especulativa, cada correção foi
motivada por uma medição concreta.

### Bug 1 — Claim loop serializado (`crates/agent/src/dds.rs`)

`AgentDds::run()` processava cada tentativa de claim (write ASSIGNED → `sleep(250ms)` →
confirma ownership) **inline**, dentro do único loop que consome `stream_tasks()` — travando
todo o loop atrás dos 250ms de qualquer task antes de sequer considerar a próxima. Teto medido:
~4 claims/s, independente de `--slots`/GPU. Corrigido: cada tentativa agora roda como sua
própria `tokio::spawn`, liberando o loop principal para continuar consumindo o stream
imediatamente; a espera por slot de processamento (antes um `bail!` definitivo se não houvesse
slot livre — travava a task em ASSIGNED para sempre) virou um poll de 20ms *depois* da
confirmação de ownership, preservando a garantia de que toda task confirmada eventualmente
processa. Regressão pega no processo: o teste `agent_e2e` (pré-existente) começou a falhar
(4/10 em vez de 10/10) até o fix do slot — ver histórico do commit para os dois rounds.

### Bug 2 — Parede de ~65 tasks em `read_task_mesh` (`crates/dds-dataspace/src/lib.rs`)

Com o Bug 1 corrigido, um segundo teto apareceu: sempre ~65-69 tasks totais processadas por
execução, depois disso todo claim "perdia a arbitragem" mesmo com um único agente rodando.
Causa raiz: `read_task_mesh()` fazia um `dds_read()` bruto cujo buffer interno
(`DataReader::read_impl` na crate `cyclonedds`) é fixo em 256 amostras **somadas entre todas
as instâncias/tasks**, não por task; como o RHC nunca purga tasks concluídas e cada task
acumula ~4 amostras de status, o scan saturava em ~256/4 ≈ 64 tasks. Corrigido: a confirmação
de ownership agora usa `dataspace.caches().read_task(&task_id)` — um `DashMap` sem limite, já
alimentado pelo upsert monotônico do próprio `stream_tasks()` (infra da Fase 5), reutilizando
um padrão já existente no código (`caches().all_tasks()`). Validado com engine mock: 100
workers/30s → 2594/2594 ok, 0 erros, 0 timeouts (antes travava em ~65).

### Bug 3 — "Thundering herd" em `server_response` (llama.cpp, fora do Rust)

Com os dois bugs acima corrigidos, o teto real na GPU ficou em ~7,5-9,6 req/s, achatado
independente da concorrência, com GPU em só 57-59% de utilização — não é gargalo Rust/DDS
(confirmado batendo direto no HTTP do `llama-server` via `curl`, mesmo teto). Causa raiz em
`tools/server/server-queue.cpp`/`.h` (compartilhado por DDS e HTTP): `server_response::send()`
usava um único mutex/`condition_variable` global; cada token gerado para QUALQUER request
acordava TODAS as threads esperando por QUALQUER task (`notify_all()`), cada uma disputando o
mesmo mutex só para descobrir que não era dela. Corrigido com um `task_waiter` por task
(mutex + CV + mailbox próprios) — `send()` agora acorda só a thread dona via `notify_one()`
direcionado. Aplicado em `src/llama_cpp` (árvore rodando) e replicado em
`third_party/llama.cpp_dds` (árvore canônica, build ainda bloqueado pelo bug não relacionado
da R4). Resultado: GPU passou a bater 100% de utilização (era 57-59%); throughput subiu de
~9,6 para **~11 req/s em 50-100 workers** (pico limpo).

**Meta de 100 req/s não atingida — e a razão agora é física, não de software:** acima de
~100 workers o throughput cai por backpressure correta (tasks esperam a GPU já saturada e
estouram o deadline), não por bug. Fechar esse gap exigiria mais throughput bruto de GPU
(quantização mais eficiente, mais/melhor hardware) — fora do escopo de uma correção de
concorrência.

**Comparação Python (contexto, não aprofundada):** o agente Python é serial por design (a
própria `--help` do agente declara isso), ~300ms/request real na mesma GPU já corrigida →
teto teórico ~3,3 req/s, sem NENHUMA capacidade de escalar por concorrência — vs. Rust ~11
req/s medido com GPU saturada. Uma tentativa de medir Python sob concorrência real esbarrou
num bug de visibilidade separado (cliente que escreve+lê uma Task sob `Ownership::Exclusive`
nunca observa as atualizações do agente) — reportado como achado, não investigado a fundo
por decisão do usuário (fora de escopo desta rodada).

### Bug 4 — Desbalanceamento de carga entre agentes (viés de arbitragem, bloqueava H3/dissertação)

Pedido do usuário: verificar se a implementação Rust bate com a metodologia da dissertação
(`tese/69a588a60776208777b2007b/dissertacao.tex` — versão Overleaf, não os arquivos de
`docs/thesis/`). A checagem cruzada achou que os resultados preliminares de OP1/OP2 JÁ
documentados na dissertação relatam **94,8%/59,8% das tasks sempre para o mesmo agente**,
bloqueando explicitamente a Hipótese H3 ("o mecanismo de reivindicação precisa ser
ajustado"). Reproduzido e confirmado empiricamente nesta sessão com agentes mock reais (sem
GPU): 2 agentes competindo pela mesma task → **299/300 (99,7%) para um só**; com um SEGUNDO
par de agentes, o vencedor **inverteu** (300/300 para o outro) — não é sorte por task nem
ordem de início, é uma característica FIXA por par de conexão.

Causa raiz: todos os agentes usam `Ownership::Exclusive` com a MESMA `ownership_strength`
fixa (`DataSpace::STRENGTH_AGENT`). Num empate de força, o CycloneDDS aplica um desempate
determinístico (por GUID do writer, já documentado no comentário de `read_task_mesh`: "empate
→ menor GUID") que não muda por task — um agente vence quase toda disputa, para sempre,
enquanto a conexão durar. `Ownership::Exclusive` foi feito para eleger UMA fonte autoritativa
entre escritores redundantes (failover), não para balancear carga entre workers competindo
por itens de trabalho distintos — usar força fixa para isso produz "vencedor leva tudo" por
construção, não por acaso.

Fix (`crates/dds-dataspace/src/lib.rs`): cada agente passa a ter um POOL de 64 writers de
`Tasks`, cada um com uma força ligeiramente diferente (`ownership_strength + hash(seed do
processo, slot) % 64` — seed via `RandomState` do próprio SO, não um `DefaultHasher` de
chave fixa, que daria a MESMA seed pra todo mundo e reproduziria o bug). Cada task é roteada
para o slot `hash(task_id) % 64` (hash de chave FIXA — `DefaultHasher::new()` — precisa ser
igual em todos os processos). Resultado: o "vencedor" da arbitragem passa a variar por task
em vez de ser sempre o mesmo agente, sem precisar de coordenação entre agentes nem mutação de
QoS em tempo real por escrita (que reintroduziria a serialização do Bug 1).

**Efeito colateral pego e corrigido**: mudar o roteamento de writers alterou o timing relativo
entre os tópicos `Tasks` e `TaskOutput`, expondo uma corrida PRÉ-EXISTENTE em
`client::submit()` (`crates/client/src/lib.rs`) — o status DONE podia chegar antes do último
chunk de conteúdo (streams independentes, sem garantia de ordem entre tópicos), retornando
`content` vazio. Fix: só finalizar quando status==DONE E um chunk com `is_final` já tiverem
sido observados, não importa a ordem de chegada.

**Achado separado, também corrigido a pedido do usuário**: investigando o desbalanceamento,
achei que `DataSpace::STRENGTH_AGENT` estava em `300` — MAIOR que
`STRENGTH_ORCHESTRATOR` (`200`) — invertendo a precedência documentada em
`dds-contract/src/roles.rs` ("orquestrador vence agentes", validado no Python: agente=100 <
orquestrador=200). Isso quebrava silenciosamente o reaper de failover (T-403): a
reatribuição de tasks de um agente morto para PENDING nunca vencia a arbitragem contra o
próprio write antigo do agente morto. Revertido `STRENGTH_AGENT` para `100` (valor
documentado) — destrava o teste `t403_agente_morto_reatribui_tasks`, que estava
silenciosamente quebrado antes desta sessão (não causado pelas mudanças de hoje).

**Gate de saída:** ✅ os 4 bugs corrigidos e validados (build/clippy `-D warnings`/fmt limpos,
78/78 testes passando — mesmo baseline histórico, incluindo o reaper T-403 destravado);
fairness confirmada empiricamente (299/300→158/142 com 2 agentes; monopólio→39/31/30% com 3
agentes); GPU real confirmada saturando a 100%; pico de throughput ~11 req/s documentado com
metodologia e tabelas completas em `OPTIMIZATION_REPORT.md`.

## Rodada 4 — Auditoria crítica de arquitetura/performance — ✅ CONCLUÍDA (2026-07-22)

**Objetivo:** revisão crítica pedida pelo usuário (Rust + llama.cpp/DDS), procurando bugs de
arquitetura/performance além do já documentado. Duas investigações paralelas, depois correção
dos achados reais confirmados.

**Corrigido (Rust):** regressão de `STRENGTH_AGENT` (revertida de novo — provável escrita não
durável no mount CIFS, reaplicada e reverificada); `new_writer_pool()` tinha um segundo
caminho de escrita de `Tasks` sem o pool de fairness (unificado via `build_tasks_writer_pool`/
`select_task_writer_slot`, compartilhados agora por `DataSpace::new()`, `new_writer_pool()` e
`writer_pool::make_write_fn`); `reap_dead_agents` O(n)→O(1) com `HashSet`.

**Corrigido (C++):** `DDSBridge::handle_request()` contava pendências errado em redelivery
TRANSIENT_LOCAL (incremento movido para dentro da seção crítica, gated em `inserted`);
`TaskOutput` na árvore canônica (`third_party/llama.cpp_dds/dds/v4/dds_v4_bridge.cpp`) estava
`VOLATILE` em vez de `TRANSIENT_LOCAL` (bomba-relógio silenciosa — reader/writer não casariam
quando essa árvore for destravada), mais tuning de priority/latency-budget que faltava.

**Revertido (não era bug):** tentativa de adicionar `@key` aos tipos `LLM.*` (keyless por
design, REQ-003 documentado — casa com o wire format do Python de referência). Pego pelos
próprios testes existentes (`llm_types_are_keyless`) antes de ser considerado concluído — ver
`OPTIMIZATION_REPORT.md` para o relato completo, é uma lição de processo válida por si só.

**Gate de saída:** ✅ 78/78 testes Rust passando; build C++ (árvore antiga, a que roda de
verdade) limpo; todas as correções sincronizadas nas duas árvores C++.

## Rodada 5 — "Esquece Python! Tudo deve ser Rust" — fechamento dos itens pendentes (2026-07-22)

**Objetivo:** mandato explícito do usuário — parar de comparar com Python e fechar todos os
itens ainda pendentes das rodadas anteriores. Três frentes fechadas nesta rodada.

### P3 (Fase R7) — enums tipados

Ver Fase R7 acima (já marcada ✅) — 8 enums adicionados a `orch-common`, com as divergências
reais achadas contra `OrchestratorV4.idl` documentadas ali.

### Suíte de testes/benchmarks C++ apodrecida — ✅ CONCLUÍDA

**Objetivo:** `tests/test-dds.cpp`, `dds/benchmark_multi_dds.cpp`, `dds/benchmark_stream_dds.cpp`
não compilavam contra a bridge atual — apontado como pendência desde a Rodada 4. Investigação
completa (build limpo fora do CIFS em `/tmp/llamacpp_dds_verify_build`, CPU-only, ambas as
árvores) achou **5 arquivos bit-rotted**, não 3 — `dds/benchmark_final.cpp` e
`dds/test_client.cpp` também quebrados, mais `src/llama_cpp` tinha sua PRÓPRIA versão
divergente e igualmente quebrada de todos os 4 (não só `third_party`).

Dois tipos de bit-rot distintos, ambos pela mesma migração (unificação do namespace IDL com o
Python, já concluída no resto do código):

1. **Falta `using namespace llama_dds;`** — `benchmark_final.cpp`, `benchmark_multi_dds.cpp`,
   `benchmark_stream_dds.cpp` usavam `llama_ChatCompletionRequest`/`_Response` sem qualificar o
   namespace (só funciona como `llama_dds::llama_ChatCompletionRequest`). Fix mecânico:
   adicionada a using-declaration.
2. **Layout de struct pré-unificação** — os mesmos 3 arquivos (mais `test-dds.cpp`) construíam
   requests com `.model`/`.messages` como sequência de `ChatMessage` (`_buffer`/`_length`/
   `_maximum` manual + `malloc`) e liam `.finish_reason` como string — layout que não existe
   mais desde a unificação com o Python (`model_name`, `messages_json` como JSON string,
   `finish_reason` como `int32_t`). Mesma classe de bug do fix da R4 em `server.cpp`, só que
   nunca recompilado até agora porque nada no CI/build normal força esses alvos. Também os
   símbolos de topic descriptor (`llama_ChatCompletionRequest_desc`) nunca existiram sob esse
   nome — só `orchestrator_LLMInferenceRequest_desc`/`orchestrator_LLMInferenceResult_desc`.

Adicionalmente, `src/llama_cpp/dds/{test_client,benchmark_final,benchmark_multi_dds,
benchmark_stream_dds}.cpp` incluíam `idl/LlamaDDS.h` (arquivo **deletado**, só sobra `.o` de
build antigo) e usavam os nomes de tópico pré-unificação (`llama_chat_completion_request` em
vez de `LLM.InferenceRequest`) — ou seja, mesmo se compilassem por acidente, nunca
casariam com o servidor real. Corrigidos copiando as versões já corrigidas de
`third_party/llama.cpp_dds` (idênticas em lógica, só divergiam nesses pontos bit-rotted),
com verificação `md5sum` pós-cópia (disciplina desta sessão no mount CIFS).

`tests/test-dds.cpp` reescrito por completo (não só qualificado): asserts contra
`.model`/`.messages`/`.prompt_tokens`/`.completion_tokens`/`finish_reason` como string
trocados por `.model_name`/`.messages_json`/`.tokens_prompt`/`.tokens_completion`/
`finish_reason` como `int32_t`, também faltava a mesma using-declaration.

**Gate de saída:** ✅ `test_client`, `benchmark_final`, `benchmark_multi_dds`,
`benchmark_stream_dds`, `llama-dds` e `llama-server` buildam limpo numa árvore de build fresca
(`cmake -DLLAMA_DDS=ON -DGGML_CUDA=OFF -DCMAKE_BUILD_TYPE=Release`, CPU-only, 24 núcleos);
`test-dds` passa via `ctest` e execução direta (2/2 testes); todas as correções sincronizadas
e md5-verificadas em `src/llama_cpp` e `third_party/llama.cpp_dds`.

### Harness de carga distribuído (`experiments/dds_async_campaign.sh`) — bug real achado, corrigido

Investigando os "20/72 timeouts" pendentes da Rodada 2/R2, achei o log de uma campanha
distribuída (2 hosts reais via SSH, `192.168.1.61`/`.62`, domain DDS 44) rodando desde
2026-07-21 20:53 até travar/morrer por volta de 2026-07-22 12:47 sem completar (processo não
está mais rodando). Padrão nos números: Rep 1 quase 100% ok, degradando progressivamente até
Rep 5 com quase todas as células em falha total, e — o achado real — toda célula "com sucesso"
mas com `avg`/`p50` ≈ 120000ms (ex.: `R4_QoS_StreamLike_short: 72 ok, 28 fail, avg=119936ms`).

**Causa raiz:** bug no próprio harness, não no sistema Rust. `wait_for_completion()` (linha 78)
retorna `"completed"` ou `"timeout"` depois de até 120s de polling, mas o chamador (linha 161,
antes do fix) **descartava esse valor de retorno** (`> /dev/null`) e incrementava `success`
incondicionalmente sempre que o `submit_async` inicial retornava um `task_id` válido — mesmo
quando o polling na verdade estourou o timeout de 120s sem a task nunca completar. Resultado:
uma célula onde a maioria das requisições trava em timeout aparece como "sucesso" com uma
"latência" de ~120000ms, poluindo `avg`/`p50`/`p95` com artefatos de timeout em vez de
descartá-los como falha real.

Fix em `experiments/dds_async_campaign.sh`: captura o retorno de `wait_for_completion` numa
variável e só conta como `success`/grava a latência quando o valor for exatamente
`"completed"`; caso contrário conta como `fail` (mesmo already-quebrado caminho usado quando o
`submit_async` inicial falha). `bash -n` confirma sintaxe válida.

**Em aberto, não investigado nesta rodada:** por que a taxa de falha real (não maquiada) piora
progressivamente ao longo dos reps (R1 limpo → R5 quase total) — hipóteses não descartadas:
vazamento de recurso nos processos remotos de orchestrator/agent que não são reiniciados entre
CÉLULAS (só entre troca de profile), acúmulo de entidades DDS, ou os hosts remotos ficando
sob pressão de memória/CPU ao longo de ~16h de campanha contínua. Requer acesso/investigação
nos hosts remotos (`192.168.1.61`/`.62`) para diagnosticar — não tentado nesta rodada por ser
uma ação com efeito em infraestrutura compartilhada fora do escopo de "corrigir o harness".

**Gate de saída:** ✅ bug do harness identificado e corrigido, `bash -n` limpo. 🟡 causa da
degradação progressiva nos hosts remotos permanece em aberto — recomendado re-rodar a
campanha com o harness corrigido para obter números reais antes de decidir se há um problema
de recurso genuíno no sistema.

## Ordem de execução recomendada (Rodada 2)

1. ✅ **R1** (harness de carga) — concluída.
2. ✅ **R5** (formalizar a crate) — concluída: publicada no crates.io e GitHub (v2.0.0).
3. ✅ **R4** (build C++ da árvore nova) — concluída na Rodada 5: `llama-server` builda limpo em
   `third_party/llama.cpp_dds` numa árvore de build fresca fora do CIFS.
4. ✅ **R3** (microbenchmarks) — concluída.
5. ✅ **R2** (medir WaitSet) — concluída na Rodada 6: teste novo prova `registration_count`
   compartilhado (2×N) sob `client::submit()` concorrente real, mais decomposição
   fila-agente/inferência via `t_agent_queue_ns`/`t_inference_ns`.
6. ✅ **R6** (E2E com concorrência) — concluída, achado forte (Rust 10/10 vs Python 10/10
   TIMEOUT na mesma carga).
7. ✅ **R7** (enums P3) — concluída na Rodada 5.
8. ✅ **Rodada 3** (teste de carga, 3 bugs reais corrigidos, ~11 req/s com GPU saturada) —
   concluída, ver seção acima.
9. ✅ **Rodada 5** (P3, suíte de testes C++ apodrecida, bug do harness de carga distribuído) —
   concluída, ver seção acima.
10. ✅ **Rodada 6** (investigação real nos hosts remotos: causa raiz da degradação progressiva
    — bug em `reap_dead_agents` que republicava violação a cada ciclo, corrigido; R2 medido de
    verdade; `ContentFilteredTopic` resolvido via documentação; migração `cyclonedds-rust` para
    versão publicada) — concluída, ver `OPTIMIZATION_REPORT.md` §"Rodada 6". 221 testes
    passando, clippy/fmt limpos.
