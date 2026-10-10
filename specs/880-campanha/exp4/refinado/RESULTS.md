# EXP4 refinado — injeção log-driven (resultados, 2026-10-05)

**Protocolo:** kill -9 da vítima na 2ª "task concluída" do seu log (fim de B = início de C — o ponto de falha da dissertação §3.7.7), janela do cliente 60 s, timeout = desfecho de segurança. 30 runs.

## Resultados

| Desfecho | Valor |
|---|---|
| Injeção mid-flight efetiva (kill durante estágio, ≥200 ms até o fim) | **6/30** |
| Injeção pós-conclusão (kill após o fim do fluxo — sem efeito) | 9/30 |
| Vítima com <2 estágios concluídos (distribuição racy — sem gatilho) | 15/30 |
| Recuperação mid-flight | 531 / 1038 / **13436 / 13681 / 14389 / 15027 ms** — **bimodal**: 2 rápidas (<1,1 s) e 4 lentas (13,4–15,0 s) |
| **T̃ref (mediana mid-flight)** | **≈ 13,7 s** |

## Causa-raiz da cauda lenta (medida e explicada)

A reatribuição strength-10 do reaper/republisher **só fica visível quando o writer morto é destruído no DDS** (relinquish_ownership). Com processo morto (kill -9), a destruição depende do ciclo SPDP/lease (~10–15 s observados) — e o kill às vezes só se torna "dono liberado" depois disso. Os 2 casos rápidos (<1,1 s) = corrida em que o saudável reclamava a task já reatribuída no momento do kill (janela de claim de ~500 ms).

## Leitura para a dissertação (RQ4)

1. **Recuperação FUNCIONA (30/30... 6/6 mid-flight completaram)** — mas o custo é o ciclo de **lease + relinquish do DDS** (~13–15 s), não a detecção (reaper ~2 s) nem o re-claim (~0,5 s).
2. **T̃ref ≈ 13,7 s (bimodal)** → margem H3 = max(0,20·T̃ref, 500 ms) ≈ **2,7 s** (a congelar).
3. **Caminhos de mitigação identificados** (fase futura): (a) reaper escreve PENDING com strength 200 transitório para forçar o RHC (limpa o dono morto imediatamente), (b) `autodispose`/unregister via API, (c) liveliness ManualByTopic na Tasks p/ os agentes (o dono morto é removido no lease curto da task). Cada um muda o contrato/protocolo — decisão de design, não bug.
4. Taxa de injeção efetiva 6/30 (20%) limitada pela distribuição racy de tasks entre 2 agentes — para os 30 runs definitivos, forçar a vítima a claimar (target_agent na vítima) via `--dispatch-mode` + registro por agente.
