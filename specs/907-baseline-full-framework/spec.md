# Especificação — 907: Baselines full-framework (LangGraph + MAF) canônicos no repo × DDS-LLM-Orchestrator

Status: EXECUÇÃO autorizada pelo líder (2026-10-26). Autoridade: pedido direto do autor
("usar o framework LangGraph e MAF **completos**, com todas as funções do orquestrador,
para comparar com o meu orquestrador; código em `src/rust` por ser o repositório git;
Python permitido para LangGraph/MAF; ignorar B-BENCH; foco em paridade; SDD").

## Contexto e posicionamento honesto

- A campanha confirmatória `specs/880-campanha/` **já foi executada** (EXP0–EXP4 + EXP-C,
  TOST formais; REPORT 2026-10-06) com o harness Python em `benchmarks/orchestration/` —
  FORA do repositório git. Esta fase **não repete** a 880 às cegas: torna o harness de
  baselines **canônico dentro do repo**, alinhado ao protocolo congelado
  (`specs/860-backends/manifest-freeze.md`), e o estende ao streaming (única função do
  orquestrador sem cobertura 880).
- Os adaptadores MAF/LangGraph são Python de nascença (REPORT 880, "Nota de método"):
  o baseline é o framework exato; Rust = sistema, C++ = inferência, Python = baseline
  alheio + instrumento de comparação. Reimplementar os frameworks invalidaria a comparação.

## Matriz de paridade (fonte dos REQ-002..009)

| # | Função do orquestrador (Rust/DDS) | LangGraph | MAF | Status |
|---|---|---|---|---|
| F1 | Workflow seq A→B→C | `StateGraph` | `WorkflowBuilder`+`Executor` | PAR |
| F2 | Fan-out/fan-in (forkjoin) | ramos nativos | fan-out/fan-in edges | PAR |
| F3 | Fork-join serial (p=1) | `max_concurrency=1` | paralelismo 1 | PAR |
| F4 | Recuperação fail-once + retomada (reaper/TTL/reassign) | checkpointer + `thread_id` | checkpoint/resume de superstep | PAR |
| F5 | Retry com teto (`max_retries`) | `RetryPolicy` | re-execução do executor | PAR (semânticas documentadas, não forçadas a coincidir) |
| F6 | Concorrência c∈{1,2,4} com isolamento por sessão | grafos paralelos + thread_id | executores concorrentes | PAR |
| F7 | Streaming de saída (TTFC) | `astream`/eventos | `run_stream` | PAR-ou-PARCIAL com evidência; **proibido simular** |
| F8 | Contexto entre estágios (context-store) | estado do grafo | shared state / mensagens tipadas | PAR |
| F9 | Claim distribuído DDS + seleção de agente + QoS dinâmica NFCM | sem nativo | sem nativo | **DIFERENCIAL DDS** — exercitado só no lado DDS (células 1 e 2 agentes em claim) |

## Requisitos e aceites

| ID | Requisito | Given / When / Then | Marco |
|---|---|---|---|
| REQ-907-001 | Baselines canônicos no repo | Dado o repositório `src/rust`, quando clonado, então `baselines/py` contém pacote autossuficiente (contratos, assembly, prompts, fixtures, providers, adapters) que roda MAF e LangGraph **completos** sem depender de caminhos fora do repo. | M1 |
| REQ-907-002 | Paridade seq | Dado `seq_chain_v1`, quando executado nos 3 sistemas com o mesmo client, então A→B→C com as MESMAS mensagens (hash de request idêntico por estágio). | M1 |
| REQ-907-003 | Paridade fork-join | Dado `fork_join_v1`, quando executado, então fan-out A∥B + fan-in C canônico único nos 3 sistemas; serial (`fork_join_serial_v1`) prova não-sobreposição. | M2 |
| REQ-907-004 | Paridade recuperação | Dada falha injetada 1× em C após B salvo, quando retomado, então LangGraph retoma via checkpointer e MAF via checkpoint/resume, sem reexecução de A/B (contagem de chamadas por estágio = esperado). | M2 |
| REQ-907-005 | Paridade retry | Dada falha transiente com retry habilitado, quando o estágio falha 1×, então o framework repõe dentro do teto e o rastro de tentativas é registrado; semelhança com `max_retries` do DDS documentada como análoga, não idêntica. | M2 |
| REQ-907-006 | Paridade concorrência | Dado c∈{1,2,4} workflows simultâneos, quando executados, então isolamento por sessão (`thread_id`/instância) sem cross-talk e makespan/p95 medidos igualmente nos 3 sistemas. | M3 |
| REQ-907-007 | Streaming com evidência | Dado o modo stream, quando executado, então TTFC é medido do evento nativo do framework (LG `astream` events / MAF `run_stream` / DDS chunks de `TaskOutput`); se a API não expuser chunk a chunk, o cenário é marcado PARCIAL no REPORT com a evidência, nunca simulado. | M3 |
| REQ-907-008 | LLM real compartilhado | Dado `--llama-url`/`--model` (default gemma-4-12b :8083, configurável), quando qualquer sistema executa, então o MESMO modelo/servidor recebe as MESMAS mensagens com sampling fixado (temp 0 + seed por run) e o uso de tokens é registrado. | M3 |
| REQ-907-009 | Lado DDS inalterado | Dado o lado DDS, quando a comparação roda, então usa bins existentes SEM modificá-los: `orchestrator` HTTP `/api/v1/chat/completions/sync` (T1–T6), `agent --engine http`, `client/submit-one`; células com 1 agente (dedicado) e 2 agentes (claim distribuído). | M3 |
| REQ-907-010 | Driver de campanha | Dada a campanha, quando disparada, então ordem intercalada round-robin entre sistemas, warmup ≥10 descartadas, ≥30 runs/condição, manifest por run (SHA do gguf, versões dos frameworks, commit do repo) e JSONL por run com relógio monotônico. | M4 |
| REQ-907-011 | Análise honesta | Dados os JSONL, quando analisados, então percentis p50/p95, TOST (α=0,05, IC 90%), Hodges–Lehmann e MEIs da Tab. 7 da dissertação (±20 ms; ±0,10 fração; ±15% vazão; ±20% p95) são aplicados; **nenhuma afirmação de vitória sem TOST**. | M4 |
| REQ-907-012 | Diferenciais declarados | Dado o README, quando lido, então F9 está declarado como diferencial do DDS (sem equivalente nativo nos frameworks), não como função "parada à força" nos baselines. | M1 |
| REQ-907-013 | Artefatos fora do repo | Dado um run, quando concluído, então resultados vão para `benchmarks/orchestration/results/<run_id>/` (fora do git); nada de artefato é commitado em `baselines/`. | M1 |
| REQ-907-014 | Equivalência estrutural com LLM real | Dado LLM real (não determinístico), quando comparado entre sistemas, então o critério é estrutural (mesmas mensagens/ordem/modelo — hash de request), mantendo os testes byte-idênticos do EXP0 dentro do repo via client determinístico. | M2 |

## Invariantes

- Semântica at-least-once no DDS; `retry_count` não é fencing; nenhuma claim de exactly-once.
- Protocolo alinhado ao congelamento 860 (unidade experimental = run independente; warmup;
  blocos; sanity em todo JSONL). Runs novos geram manifest próprio — `campaign_data/` não é fonte.
- Nenhum arquivo dos crates de runtime é modificado (T906 em execução neste tree).
- Commits `[T-907-XX]`, um por task; sem commit em `main`.

## Marcos

- **M1** — repo autossuficiente + paridade seq (REQ-001/002/012/013).
- **M2** — forkjoin/serial + recovery + retry + equivalência estrutural (003/004/005/014).
- **M3** — concorrência + streaming + LLM real + lado DDS (006/007/008/009).
- **M4** — driver de campanha + análise + EXECUÇÃO EXP1–EXP4 n≥30 + REPORT (010/011).
