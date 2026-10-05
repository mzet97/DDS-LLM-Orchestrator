# EXP4 — piloto T̃ref e diagnóstico de recuperação (2026-10-05, DDS)

**Protocolo:** dissertação §3.7.7 — falha real (morte do processo) com task em mãos, agente saudável herda. T̃ref (mediana de recuperação da referência) será congelado antes dos 30 runs definitivos (Tab.11: "piloto informa variabilidade, não altera MEI pela direção do efeito").

## Setup do piloto
Domínio 189, loopback: orquestrador + `agent-saudavel` (mock, sempre vivo) + `agent-vitima` (mock). Fluxo A→B→C via `wf-run`; a vítima é `kill -9` em t+0,9 s (com estágio em execução). Recuperação = reaper (stale 2 s) → reatribuição strength-10 + **republisher com renovação de `created_at_ns`** → claim pelo saudável.

## Resultados do piloto (5 repetições)

| rep | status | t_kill → fim |
|---|---|---|
| 1 | **timeout do cliente (15 s)** | >15 s |
| 2 | completed | **663 ms** |
| 3 | completed | **661 ms** |
| 4 | completed | **667 ms** |
| 5 | **timeout do cliente (15 s)** | >15 s |

**T̃ref candidato ≈ 0,66 s (mediana dos completos)** — mas **2/5 (40%) excederam a janela de 15 s do cliente**: recuperação com cauda pesada não-determinística.

## Diagnóstico da cadeia de recuperação (achados desta fase)

1. **Caminho rápido (663 ms):** reaper detecta (stale 2 s) → reatribui (strength 10; o writer da vítima morta foi destruído → `relinquish_ownership` no RHC) → saudável claima (100 > 10) → completa.
2. **SIGSTOP/agente congelado = buraco real (P1):** processo vivo não destrói o writer → o dono (strength 100) sobrevive ao lease na prática → a reatribuição strength-10 **nunca fica visível** → task starva para sempre. Confirmado com diagnostic run (reatribuição única, recovery nulo em 40-90 s). Mitigações a desenhar (fase futura): reaper com strength 200 temporário para "limpar" o dono + republish, ou `unregister` por API, ou política de deadline-miss → FAILED.
3. **Cauda >15 s no kill-based (2/5):** hipóteses — corrida entre o lease do DDSI e o primeiro republish (a 1ª reatribuição é rejeitada enquanto o dono existe; as seguintes dependem do tick de 2 s + destruição efetiva do proxy), ou `is_eligible` por idade em reentregas. Requer instrumentação do republisher (contador de rejeições) antes dos 30 runs definitivos.
4. Correção aplicada nesta fase: republisher renova `created_at_ns` a cada re-publicação (T-880/EXP4) — necessário para o caso em que a visibilidade só chega após o lease.

## Implicações para os 30 runs definitivos
- Aumentar a janela do cliente wf-run para 60 s (medir a recuperação real, não o timeout do cliente) OU registrar timeout-do-cliente como falha do sistema (visão do usuário — legitima e mais honesta; **decidir no congelamento**).
- T̃ref ≈ 0,66-1,0 s (mediana dos completos) → margem H3 = max(0,20·T̃ref, 500 ms) = 500 ms.
- Registrar a taxa de cauda (>15 s) como desfecho de segurança, não como falha do método.
