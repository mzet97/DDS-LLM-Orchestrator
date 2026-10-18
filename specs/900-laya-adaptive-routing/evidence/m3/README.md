# Evidências M3 — capabilities e claim adaptativo (T-900-M3-01..03)

Data: 2026-10-17 (UTC).

## Entregas

| Tarefa | Arquivos | Aceite |
|---|---|---|
| M3-01 elegibilidade | `workflow-engine/src/eligibility.rs` (+8 tests) | Anúncio validado (protocolo/catálogo/rótulos/expiração) + registry (saúde/slots/modelo) + requirements; **janela de revogação** revalidada no claim (anúncio novo sem capability revoga imediatamente); `ModelSpecialization` ≠ capability |
| M3-02 barreira opt-in | `agent/src/adaptive.rs` (+11 tests) + wiring `AgentDds::claim_eligible` (ANTES do enqueue e REVALIDADO pré-write ASSIGNED) + flags `--adaptive/--adaptive-capabilities/--adaptive-domain` + feeders/auto-anúncio (dds) | V05: sem capability → Block; V06: ordem permutada Plan/State/Task → Block até completar; **target exato** (prefixo vazio do legado NÃO autoriza — achado A04 corrigido no modo adaptativo); tentativa ativa via task_id determinístico `laya1-sha256(wf\|rev\|node\|attempt)`; tasks legadas → `Legacy` (caminho clássico intocado) |
| M3-03 contenção/restart | testes `contention_k2_*`/`contention_k4_*`/`restart_recovers_*` | K=2: só o capaz passa; K=4: todos capazes disputam a MESMA tentativa (arbitragem = protocolo claim legado), sem slots → fora; restart: barreira é estado DERIVADO dos tópicos (replay em qualquer ordem) |

## Gates (2026-10-17)

- `cargo test -p agent -p workflow-engine -p dds-dataspace` (mock): **80/0**
  (agent 28 incl. 11 novos + 17 legados verdes; wf-engine 22; dataspace 30).
- clippy `-D warnings`: OK nas 3 crates (mock e dds); fmt OK.
- `cargo build -p agent --features dds`: bin `agent` com `--adaptive` OK.

## Isolamento (D-ISOLATION, decisão de implantação confirmada em código)

- Tópicos adaptativos só existem no domínio passado por `--adaptive-domain`;
  agentes legados não assinam esse domínio → não recebem tasks laya1-.
- Sem `--adaptive`: zero consulta à barreira (comportamento byte-idêntico ao
  legado — EXP1b/EXP4 preservados; 17 testes legados do agent verdes).

## Não verificado aqui

- Contenção em DDS REAL com 2/4 processos agente disputando via protocolo de
  claim completo (exigiria engine/materializer publicando tasks — M5); a
  barreira em si é validada in-memory + a integração DDS compila e anuncia.
- V05/V06 em DDS real: idem, dependem do materializador (M5-03) para publicar
  Plan/State/Task de ponta a ponta.
