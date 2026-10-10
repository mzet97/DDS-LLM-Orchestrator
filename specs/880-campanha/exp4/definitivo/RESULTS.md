# EXP4 definitivo — injeção forçada (resultados, 2026-10-06)

**Protocolo:** kill -9 da vítima na 2ª "task concluída" (fim de B = início de C — ponto de falha da dissertação §3.7.7), **vítima forçada a claimar** (`wf-run --target-agent agent-vitima` + `agent-saudavel --target-agent-prefix agent-saudavel`; reaper limpa `target_agent` na reatribuição — commit `25998a1`), janela do cliente 60 s, timeout = desfecho de segurança. Domínio 189, loopback, mock + det-responder 50 ms. 30 runs (commit de código: `[T-880] EXP4 injeção forçada`; gates 448/0).

## Resultados

| Desfecho | Valor |
|---|---|
| Injeção mid-flight efetiva | **30/30** (refinado: 6/30 — a distribuição racy era o limitante) |
| Workflows completados | **30/30** (0 timeouts do cliente como desfecho de segurança) |
| **Recuperação (latência do estágio C, p50)** | **14.948,5 ms** |
| Recuperação min / média / max | 13.961 / 14.918 / **15.026 ms** |
| Cauda > 20 s no nível do estágio | **0/30** |

**T̃ref congelado: ≈ 14,95 s** → margem H3 = max(0,20·T̃ref; 500 ms) ≈ **3,0 s**.

## Achados

1. **A recuperação é determinística e limitada pelo lease DDSI (~15 s).** Com a injeção forçada (sem corrida de distribuição de tasks), a cauda bimodal do refinado desaparece: 29/30 entre 13,96–15,03 s. A decomposição: reaper detecta em ~2 s → reatribui strength-10 → **invisível até o writer morto ser relinquished (lease DDSI ~10–15 s)** → republisher confirma → saudável claima (~0,5 s). O custo é o mecanismo de ownership do DDS, não a orquestração.
2. **Cauda aparente do rep 2 (74,5 s) é artefato do cliente, não da recuperação:** o `wf-run` ficou ~59 s em startup (descoberta DDS) antes do fluxo; o estágio C dessa rep recuperou em **14.950 ms** (dentro da distribuição). A métrica de recuperação correta é a latência do estágio C (submit→output no cliente), não o relógio do shell. 0/30 acima de 20 s no estágio.
3. **Comparação com o lado MAF (mesmo ponto de falha, 30/30):** retomada nativa por checkpoint em **p50 = 382 ms** (381–397 ms), com prova de não-reexecução de A/B (delta de chamadas por rep: A:2, B:2, C:2 — nominal + cadeia falhada; a retomada só infere C). Razão DDS/MAF ≈ **39×** — mas os mecanismos não são comensuráveis: no MAF o **driver** segura o checkpoint e re-invoca (recuperação centralizada no cliente); no DDS a recuperação acontece **no data space** (reaper + republisher + claim por qualquer agente vivo), sem ação do cliente — o preço é o lease do ownership.
4. **Implicação de design (RQ4):** recuperação autônoma no data space custa ~15 s sob kill -9 (sem graceful shutdown); mitigação documentada no PILOTO (strength 200 transitório / unregister via API / liveliness por task) segue como trabalho futuro — muda contrato/protocolo, não é bug.

## Reprodutibilidade
`run_exp4_definitivo.sh [runs=30]` (este diretório); binários `~/.cache/tese-rust-target-fd/release` (commit `25998a1`); domínio 189; `CYCLONEDDS_URI=config/dds/cyclonedds-test-loopback.xml`. Dados brutos: `wf-1..30.json` (status + latências por estágio), `resultados.txt` (log das reps), `det.jsonl`.
