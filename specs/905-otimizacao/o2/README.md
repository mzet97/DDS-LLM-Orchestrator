# O-2 — Claim sem dupla-execução + janela curta (2026-10-18)

## Estado: CONCLUÍDO — duplas 7%→0, janela 250→50 ms, p50 sequencial −40%

## Diagnóstico (corrige a hipótese de 2026-10-16)

A hipótese antiga ("confirmação lê o cache do stream sob backpressure") apontava
para o lugar ERRADO: a confirmação pós-claim já lia o RHC dirigido (`read_task_mesh`,
T-203, desde 2026-07). O consumidor do cache era a **REVALIDAÇÃO pré-claim** do
dispatcher (`caches().read_task`): alimentada pelo stream de ingestão com
backpressure (fila 4×slots), ela validava o claim do loser contra um snapshot
PENDING **obsoleto**; o loser então escrevia ASSIGNED com writer mais forte e
**sobrescrevia o estado terminal do vencedor** (Exclusive Ownership arbitra por
FORÇA, não por recência) → dupla-execução.

**Prova experimental da mecânica:** janela MAIOR piora — 250 ms → 7% duplas
(21/300); 600 ms → **13,5%** (27/200). Janela maior = cada item da fila ocupa o
slot por mais tempo = fila do loser drena mais devagar = snapshot mais velho no
momento do claim tardio. (Teoria de lag-de-propagação previa o oposto.)

## Fix

`crates/agent/src/dds.rs` (dispatcher): revalidação pré-claim agora lê
`read_task_mesh` (estado ARBITRADO do RHC, independente da fila de ingestão).
`claim_eligible` rejeita status≠PENDING no estado fresco → claim tardio nunca
sobrescreve vencedor. Com a segurança na revalidação, `CONFIRM_DELAY` default
250→**50 ms** (a janela só arbitra claims simultâneos; propagação ao RHC é ms).
Parametrizável: `AGENT_CONFIRM_DELAY_MS`.

## Números (2 agentes mock 250 ms, domínio 77, e2e-bench)

| cenário | p50 | duplas | failures |
|---|---|---|---|
| baseline (código original, janela 250) — 3×100 conc. | 2 526–2 788 ms | **21/300 (7%)** | 4 |
| experimento janela 600 (sem fix) — 2×100 conc. | 5 233 ms | **27/200 (13,5%)** | 4 |
| FIX + janela 250 — 3×100 conc. | 2 027 ms | **0/300** | 0 |
| FIX + janela 50 — 3×100 conc. | 1 227 ms | **0/300** | 0 |
| FIX + default 50 — sequencial n=30 | **306 ms** | 0 | 0 |
| FIX + default 50 — 100 conc. (validação final) | 1 227 ms | **0** | 0 |

Gates do plano: duplas ≤ 6/100 ✓ (0); p50 sequencial ≤ ~300 ms ✓ (306; baseline
506 = **−40%**); contenção sem regressão ✓. Suítes: agent 13+1+11+3 default,
14+1+1+3 dds (1i conhecido), clippy -D warnings 0, fmt ok.

## Nota honesta

A dupla-execução sob rajada NÃO era intrínseca ao at-least-once como se declarou
em 2026-10-16 — era o cache obsoleto na revalidação. A limitação declarada na
dissertação deve ser REESCRITA: com revalidação no mesh, 0 duplas em 700 tasks
medidas; residual teórico = janela de claims simultâneos (<50 ms) e particionamento
de rede (não testado aqui). At-least-once permanece a semântica do contrato (dedup
por consumidor idempotente segue recomendado), mas o caso rajada-local está fechado.

Artefatos: logs ANSI-stripped dos runs (`o2_*.log` baseline/600ms, `o2f_*` fix+250/50,
`o2g_*` default final).
