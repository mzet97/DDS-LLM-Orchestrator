# Constituição da Migração para Rust

> Regras **não-negociáveis** que governam TODO o trabalho neste workspace. Precedência
> máxima: se qualquer spec, plan ou task contradisser um artigo desta constituição, a
> constituição vence e o conflito deve ser reportado ao líder (não resolvido no escuro).

Versão 1.0 · autoridade: arquiteto/líder · executor: IA de implementação.

---

## Artigo I — Interop primeiro, big-bang nunca
1. Todo nó (Python, C++, **Rust**) fala o **mesmo wire format DDS**: XTypes / XCDR,
   gerado do **mesmo** `OrchestratorDDS.idl`. Nenhuma definição de tipo é escrita à mão
   em Rust — sempre gerada pelo `cyclonedds-idlc` a partir do IDL canônico.
2. Cada componente migra **isoladamente** e roda **lado a lado** com a versão Python nos
   mesmos tópicos. É proibido um passo que exija parar todo o sistema.
3. Todo componente migrado deve ser **A/B testável** contra o equivalente Python antes de
   substituí-lo. Rollback = desligar o nó Rust; os Python assumem.

## Artigo II — Test-first e verificação objetiva
1. **Nenhuma task é "feita" sem teste que a prove.** Escreva o teste antes ou junto do
   código. Um componente sem teste de aceite é trabalho incompleto.
2. Todo PR/entrega roda `cargo test`, `cargo clippy -- -D warnings` e `cargo fmt --check`
   verdes. Warnings de clippy são erros.
3. Paridade comportamental: quando um componente Rust substitui um Python, deve existir um
   teste que reproduz um comportamento conhecido do Python (ex.: o `qos-nfcm` reproduz os
   números do artigo). Divergência é bug até prova em contrário.

## Artigo III — Honestidade (herdada do projeto)
1. **Nunca inventar resultados, números de benchmark, ou afirmar que algo está
   implementado sem código que prove.** Um scaffold é um scaffold; diga isso.
2. Distinga sempre no texto e nos commits: **implementado** · **parcial** · **scaffold/proposto**.
3. Se faltar informação para decidir, **marque `[NEEDS-CLARIFICATION: …]`** e pergunte ao
   líder — não preencha com suposição.

## Artigo IV — Desempenho é requisito, não enfeite
1. O objetivo da migração é **remover os gargalos do GIL** (ver `CONTEXT.md §4`). Cada
   componente tem um **orçamento de desempenho** na sua spec; entregar sem medir contra o
   orçamento é incompleto.
2. Preferir, por padrão: sem alocação no hot path (loans zero-copy), `async` orientado a
   evento (WaitSet/streams) em vez de polling, estruturas concorrentes lock-free
   (`dashmap`, atômicos) em vez de lock global, e paralelismo real (`tokio`/`rayon`).
3. Nenhuma micro-otimização sem número que a justifique. Meça, depois otimize.

## Artigo V — Escopo congelado
1. `llama_cpp` **permanece em C++**. Não migrar, não reescrever. A ponte é via DDS.
2. `automation/` (Ansible) está **fora de escopo**.
3. Não adicionar funcionalidade nova durante a migração — paridade primeiro. Ideias novas
   viram itens de backlog, não entram na task atual.

## Artigo VI — Segurança e reversibilidade
1. Mudanças que afetam o cluster ou dados compartilhados exigem aprovação explícita do
   líder e um caminho de rollback documentado.
2. Segredos (ex.: inventário do cluster) nunca entram no código ou nos specs.
3. Commits pequenos e atômicos, um por task, com mensagem rastreável à task (`[T-XXX]`).

## Artigo VII — Rastreabilidade (SDD)
1. Todo trabalho nasce de uma **spec** (`spec.md` → o quê/por quê), passa por um **plano**
   (`plan.md` → como) e vira **tasks** atômicas (`tasks.md`). Nada de código sem spec.
2. Toda task cita o(s) requisito(s) que satisfaz (`REQ-XXX`). Todo requisito tem critério
   de aceite verificável.
3. O estado de cada task é mantido no próprio `tasks.md` (checkbox) e reportado ao líder.

---

**Ao executor:** leia esta constituição no início de cada sessão. Em dúvida entre
velocidade e um destes artigos, o artigo vence. Reporte, não contorne.
