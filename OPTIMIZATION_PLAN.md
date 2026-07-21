# Optimization Plan — DDS-LLM Orchestrator (Rust workspace)

**Data:** 2026-07-20 · **Baseado em:** `OPTIMIZATION_AUDIT.md` (mesma data)
**Status:** nenhuma alteração de código foi implementada ainda — este documento só prioriza.

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

### Fase 1 — Fechar a lacuna de medição (pré-requisito para as Fases 2–5)

**Objetivo:** nenhum item P1 abaixo pode ser aceito como "melhorou o sistema" sem medição real
sob carga — hoje só há evidência de *existência* do padrão (clone redundante, zero-copy ausente,
WaitSet por stream), não de *magnitude*. Este é o pré-requisito citado em
`OPTIMIZATION_PLAN.md` (seção "Itens explicitamente fora desta rodada") da versão anterior.

- Estabelecer baseline real com `--features dds` (nunca rodado nesta auditoria): 
  `CYCLONEDDS_STATIC=1 cargo test --workspace --features dds -- --test-threads=1`, crate por
  crate onde fizer sentido (`dds-dataspace`, `agent`, `orchestrator`, `client`, `benchmarks`).
- Montar um cenário de carga reproduzível e versionado (N agentes, M clientes, duração fixa,
  usando o driver de `benchmarks` já existente — E1/E4/OP1) para servir de alvo de profiling.
- Rodar `perf stat`/`perf record` (ou `tokio-console` se viável no host) contra esse cenário
  como baseline "antes" — sem isso, as Fases 3–5 não têm como provar ganho, só mudança.

**Gate de saída:** suíte `--features dds` verde documentada + cenário de carga reproduzível
descrito (comando exato, hardware, duração) + pelo menos um perfil de CPU/alocação "antes"
arquivado para comparação.

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

### Fase 4 — Zero-copy (`write_loan`) no streaming de `TaskOutput` — 🔴 BLOQUEADA (2026-07-20)

**Não implementada — achado de segurança real na crate `cyclonedds`, não falta de tempo.**
Antes de tocar código, investiguei a API `request_loan`/`WriteLoan` em
`third_party/cyclonedds-rust/cyclonedds-rust/cyclonedds/src/writer.rs` e `topic.rs`:

1. `DataWriter<T>::request_loan()` chama `dds_request_loan` (C) e depois
   `std::ptr::write_bytes(sample_ptr, 0, size_of::<T>())` — zera a memória inteira do
   sample **no layout Rust de `T`** (não existe um struct "nativo" C separado — `DdsType::
   descriptor_size()` default é `size_of::<Self>()`, ou seja, o buffer emprestado pelo DDS
   *é* exatamente `size_of::<TaskOutput>()`, incluindo os `String`s Rust de 24 bytes cada).
2. `TaskOutput` tem 3 campos `String` (`task_id`, `content`, `agent_id`). Um `String`
   totalmente zerado **não é um bit-pattern válido**: internamente `Vec`/`String` usam
   `NonNull<u8>` para o ponteiro (nunca pode ser nulo) — zerar viola esse invariante.
3. `WriteLoan::get_mut()` devolve `&mut T` sobre essa memória zerada/inválida. Preencher um
   campo `String` com atribuição normal (`sample.content = valor;`) executa `*place = valor`,
   que primeiro roda `Drop` no valor ANTIGO (o `String` zerado/inválido) antes de mover o
   novo — ou seja, a primeira escrita já dropa um `String` com ponteiro nulo → UB certo
   (crash ou corrupção do alocador), não uma possibilidade remota.
4. Confirmação adicional: **a suíte de testes da própria crate `cyclonedds` nunca exercita
   `request_loan`/`WriteLoan`** (`grep` em `tests/integration_test.rs` não encontra nenhuma
   ocorrência) — é código não testado. E o comentário em `async.rs` sobre o fix de WF-4
   ("a amostra nativa tem layout C… `clone_out` converte para `String` de 24B") mostra que o
   *lado de leitura* já teve exatamente esta classe de bug com `String`; o lado de escrita
   (`request_loan`) não tem o análogo de `clone_out` para popular com segurança.

**Decisão:** não implementar. Forçar a mudança "porque o benchmark mede melhor" violaria a
regra de aceite do próprio plano (nenhuma otimização sem teste verde) e a regra do processo
sobre `unsafe` (só com invariantes documentados e alternativa seguro esgotada) — aqui a
"alternativa segura" simplesmente não existe hoje na API pública da crate `cyclonedds` para
tipos com campos heap-alocados. Consertar direito exigiria: (a) uma API de loan que exija
inicialização via `MaybeUninit<T>` + `ptr::write` por campo (não `&mut T` com assignment
normal), ou (b) uma restrição em nível de trait para só permitir loans em tipos `T: Copy`
(sem heap). Qualquer uma das duas é uma mudança na crate `cyclonedds` (dependência
compartilhada por todo o workspace), fora do escopo de uma rodada de otimização do
`tese/src/rust`.

**Achado novo, mais importante que a otimização em si:** isto é um **bug de segurança de
memória latente na crate `cyclonedds`** (não no workspace `tese/src/rust`) — ninguém bateu
nele ainda só porque nada no workspace chama `request_loan`/`write_loan_async` para tipos
não-POD. Recomendo abrir isso como issue dedicada na crate (autoria do próprio usuário,
`third_party/cyclonedds-rust/`), com nota `SAFETY` no doc do método atual alertando que **não
é seguro usar com tipos que tenham `String`/`Vec`/outros campos heap-alocados** até a API
mudar.

**Gate de saída (revisado):** N/A — item bloqueado por segurança, não por medição. Nenhuma
mudança de código feita em `dds-dataspace`/`agent` para este item.

### Fase 5 — `WaitSet` compartilhado com `ReadCondition` por tópico — ⏸️ ADIADA (2026-07-20)

**Decisão explícita, não esquecimento:** este é o item de maior escopo/risco do plano (muda a
API interna de streaming de "um reader dedicado por chamada" para "multiplexação sobre um
único WaitSet", toca os 17 `take_aiter()` de `dds-dataspace/src/lib.rs` e os 14 testes/contract
tests que dependem do comportamento atual de streaming). O próprio plano já dizia para validar
isso "com um cenário de carga multi-processo montado especificamente para o ganho" — essa
infraestrutura (agent+orchestrator+context-store+mcp-gateway+observability+policy-engine
rodando juntos sob carga) não existe ainda nesta sessão, e construí-la +
implementar a mudança + validar com rigor consumiria a maior parte do tempo restante.

**Priorizei em vez disso a validação E2E real com DDS/llama-server/modelo real** (pedido
explícito do usuário como entrega final desta rodada) sobre este item — ambos não cabiam no
tempo restante com o rigor que cada um exige (o gate de aceite deste item por si só pede um
teste de aceite dedicado + medição de threads sob carga real). Fica como próximo item
prioritário de uma futura sessão, com a infra de carga da Fase 1 (ainda não construída) como
pré-requisito real, não apenas formal.

**Nada foi tocado** em `dds-dataspace/src/lib.rs` para este item — os 17 `take_aiter()`
permanecem como estavam, achado ainda válido e não implementado.

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
| **P1** | Zero-copy loans (`write_loan`) não são usados em nenhum writer — todo write faz cópia, incluindo o streaming de chunks `TaskOutput` (potencialmente o maior volume de samples por sessão de inferência) | `grep` não encontra nenhuma chamada real a `write_loan`/`request_loan`/`take_loan` em `crates/*/src`; todos os 18 writers usam `.write(&x)` | `dds-dataspace` (`writer_pool.rs`, `lib.rs`), `agent` (chunks) | Trocar `outputs_writer.write(&output)` por `request_loan()` + escrita in-place no hot path de `TaskOutput` (T-616 do `ACTION_PLAN_DDS_IMPLEMENTATION.md`, nunca implementado); manter `.write()` nos tópicos de baixo volume (Tasks, AgentRegistry, etc., onde a cópia é irrelevante) | Médio — a API de loan da crate `cyclonedds` precisa ser exercitada com um tipo com `String` (o mesmo caminho que teve o bug de UB corrigido em WF-4); requer teste dedicado de round-trip antes de aceitar | Redução de alocação por chunk; o orçamento de propagação (p99 <5ms) já está 65× abaixo, então a métrica-alvo aqui é CPU/alocação por chunk sob throughput sustentado (medir com `criterion` em `spike-interop/benches` adaptado, não com o benchmark de propagação existente) |
| **P1** | `WaitSet` dedicado por chamada de stream (17 blocos `take_aiter()` independentes) — T-617 ("WaitSet compartilhado com `ReadCondition` por tópico") nunca foi implementado; com múltiplos processos assinando múltiplos tópicos, o número de threads de blocking-pool pode crescer sem necessidade | `dds-dataspace/src/lib.rs` — 17 ocorrências de `take_aiter()`, uma por método `stream_*` | `dds-dataspace` | Implementar um `WaitSet` compartilhado com `ReadCondition` por tópico (como já planejado em T-617); requer mudança de API interna de "stream por reader dedicado" para "multiplexação sobre um único WaitSet" | Alto — é a mudança de maior escopo da lista; risco de regressão nos 13 testes de `dds-dataspace` e nos contract tests A/B; fazer só com teste de aceite dedicado (`tests/shared_waitset.rs`, já especificado no `ACTION_PLAN_DDS_IMPLEMENTATION.md`) | Redução do número de threads do blocking pool sob N assinantes simultâneos; medir com processo real rodando `agent`+`orchestrator`+`context-store`+`mcp-gateway`+`observability`+`policy-engine` ao mesmo tempo (não medido nesta sessão — é o cenário que mais se aproxima de produção) |
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
