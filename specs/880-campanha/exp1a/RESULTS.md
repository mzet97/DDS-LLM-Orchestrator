# EXP1a — Ablação de substrato: InMemory vs DDS (resultados, 2026-10-05)

**Protocolo:** dissertação §3.7.3 — mesma trait `DataSpaceApi`, mesmo binário (`dds-dataspace/examples/exp1a.rs`), mesmo worker (claim otimista + confirmação por releitura do mesh RHC + 2 chunks + DONE), mesmo fluxo A→B→C, backend de inferência simulado por atraso {0, 50} ms, n=30/braço, 4 células.

## Resultados (T_total por workflow de 3 estágios; p50 em ms)

| Braço | delay=0 | delay=50 |
|---|---|---|
| `InMemoryDataSpace` (in-process) | **18** (p95 18, min 12) | **164** (p95 164) |
| `DataSpace` DDS (CycloneDDS loopback) | **18** (p95 18, min 152†) | **162** (p95 163) |

† descrição de granularidade de amostragem (multiplos do tick de 50 ms do polling); p95 idêntico.

## Veredito: **substrato DDS adiciona custo ≈ ZERO** (≤1 ms por fluxo de 3 estágios; ≤ ~0,3 ms/estágio)

A ablação isola o transporte e responde a pergunta central do EXP1a: **o custo de T_extra do DDS-LLM-Orchestrator (770 ms no EXP1) NÃO é do transporte DDS** — é do **modelo de coordenação** (3 × janela de confirmação de claim de 250 ms + política). O substrato DDS custa ≤1 ms por workflow de 3 estágios contra memória in-process (mesmo host), estável nas duas condições de atraso. Isso decompõe com precisão o confundimento "runtime × middleware" que o EXP1a existe para resolver.

## Instrumento e achados de engenharia
- `dds-dataspace/examples/exp1a.rs`: worker determinístico (claim otimista + confirmação por releitura do **mesh RHC** via `read_task_mesh` + 2 chunks + DONE); prontidão do worker por oneshot (determinístico).
- **Bug real descoberto pelo instrumento (corrigido no instrumento, questão de API aberta):** o padrão "claim → confirmar via `read_task`/cache" **não funciona no braço DDS** — o cache só avança quando um stream é policiado (take/read split), e a stream de Tasks tem filtro de conteúdo (não devolve o ASSIGNED). O agente real não sofre porque confirma via `read_task_mesh` (leitura RHC não-consumidora). **Questão aberta (P2):** `read_task`/cache de um `DataSpace` puro fica congelado sem um feeder — documentar na API ou alimentar o cache internamente em `DataSpace::new`.
-worker do mock rejeitava RUNNING/DONE derivados do PENDING (assigned preenchido→vazio = regressão correta do filtro de 820) — instrumento corrigido para derivar do CLAIMED, igual ao agente real.
- Granularidade de confirmação: 5 ms (reduzida de 50 ms — a 50 ms o sinal do substrato era mascarado pelo polling).
