# specs/870-deploy-distribuido — Deploy multi-host + prova E2E

Fonte: `PLANO_IMPLEMENTACAO_RESTANTE.md` Fase 870. Pré-requisito: tag `congelamento-pre-campanha`.

| Task | Escopo | Estado |
|---|---|---|
| T-870-01 | Estratégia de build p/ glibc 2.39: validação empírica (release local roda nos hosts — CYCLONEDDS_STATIC, símbolos ≤2.39); fallbacks não necessários | [x] |
| T-870-02 | Deploy por papel: .62 orchestrator, .61 agent, .64 2×agent, .63 wf-run+prompts; XML de descoberta (sem xmlns, id="any", multicast off, peers unicast; domínio via CLI=170) | [x] |
| T-870-03 | Provas: E2E wf-run .63→completed entre hosts; 12/12 pós-morte do agente .61; mid-flight SIGSTOP; ring final 3/3 | [x] |
| T-870-04 | REPORT + descobertas de deploy (xmlns trap, regra de domínio, pkill -x, registry sem expiry, llama .61 off) + notes.md | [x] |

Achados novos para fases futuras: registry sem expiry (P2 — correção em fase própria); XML xmlns trap documentado (vale para 880/890); llama .61 a religar para EXP2.
