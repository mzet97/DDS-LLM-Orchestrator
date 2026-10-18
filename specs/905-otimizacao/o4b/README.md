# O-4b — DONE dentro da janela de lease: PARCIAL com achado que INVALIDA a hipótese do republisher (2026-10-18)

## Estado: PARCIAL — mecanismo cliente-agente mais fundo que o esperado; fix implementado porém default-OFF; próxima sessão precisa de observador instrumentado

## O que foi feito

1. Harness EXP4-style reconstruído (`o4b_scenario.py`): submit → A clama/inicia
   inferência gemma4 (768 tok) → kill -9 A em ~1,6 s → B entra só pós-kill →
   orchestrator com fast-reaper (`--agents-stale-after-ms 3000
   --registry-interval-ms 500`) re-emfileira → B re-executa → cliente timeout 30 s.
2. Fix implementado no agente (`ensure_terminal_visible`): republish idempotente
   do terminal a cada 2 s até o readback do mesh confirmar (≤12 s ≈ lease+margem);
   `AGENT_ENSURE_TERMINAL=1` ativa (default OFF).

## Resultados do A/B (5 runs cada)

| braço | sucessos | observação |
|---|---|---|
| sem fix (`AGENT_NO_ENSURE_TERMINAL=1`) | **2/5** | sucessos a ~13,3 s (≈ lease 10 s + margem); timeouts = bug documentado |
| com fix | **0/5** | B conclui em ~11,2 s uniformes; **republisher NUNCA dispara**; cliente ainda timeout |

## Achados (mudam o diagnóstico)

1. **O republisher é no-op no cenário real**: o readback do PRÓPRIO B aceita o
   DONE dele já na primeira checagem (o RHC do B arbitra a favor de B) — não há
   o que republicar do ponto de vista do agente. A perda está ADIANTE:
   o leitor do CLIENTE não recebe o DONE (ou o chunk final — a condição de
   término do `submit` exige AMBOS: status DONE **e** chunk `is_final`).
2. B clama a task ~2 s ANTES do requeue do orchestrator (recebe o histórico
   TransientLocal como reader novo) — a recuperação por histórico antecipou a
   do reaper. O requeue posterior (write PENDING strength 200 do orchestrator,
   VIVO) pode SOBRESCREVER a instância que B já processa — suspeito principal
   da cegueira do cliente (a partir do requeue, o dono arbitrado da instância é
   o orchestrator, e o DONE de B perde a arbitração nos OUTROS readers; o
   readback do próprio B reflete o estado do SEU RHC que já havia aceitado o
   DONE antes… — inconsistência entre readers que precisa de observador
   instrumentado para fechar).
3. A diferença 2/5 vs 0/5 sugere interferência sistemática (não só sorte do
   sorteio de forças por slot — 0,5⁵=3%).

## Próxima sessão (ordem)

1. Observador instrumentado: processo dedicado imprimindo (status, assigned,
  writer-GUID inferido) da instância a cada 500 ms + todos os chunks TaskOutput
  com seq_num — resolve quem é o dono arbitrado em CADA reader a cada instante.
2. Testar hipótese do requeue-200: cenário com reaper DESLIGADO (recuperação só
   por histórico TransientLocal — B já provou clamar sozinho) → se o cliente
   passar a receber, o conflito é o write strength-200 do orchestrator sobre
   instância em processamento (fix: reaper não re-emfileira task com heartbeat
   de dono vivo, ou write de requeue com strength do papel CLIENTE).
3. Reavaliar a condição dupla do cliente (DONE + chunk final): o chunk `is_final`
   de B pode ter sido perdido na troca deOwnership do tópico TaskOutput.

## Gates

Suítes agent 21/0 (dds) verdes com o change default-OFF; build/clippy ok.
Kill criterion respeitado: fix não provou benefício → não fica ligado por padrão.
