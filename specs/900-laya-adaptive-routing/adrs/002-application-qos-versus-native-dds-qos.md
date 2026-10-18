# ADR-002 — Objetivo Laya e QoS DDS efetivo

Data: 2026-10-16. Status: PROPOSTO, requer revisão antes de M4. Laya como origem semântica é decisão fechada do usuário.

## Contexto

REQ-900-005/007/014/015. O loop legado aplica knobs ao pool de writers de Tasks; os perfis estruturais têm diferenças que afetam matching. tasks_with_profile ignora os knobs retornados pelo mapper ao construir a entidade. Evidências A06/A08 do [audit](../audit.md).

## Decisão proposta

V1 usa QoS de aplicação sobre endpoints DDS estruturais fixos; mapeamento e limites em [contracts](../contracts.md). Publicar perfil solicitado, aceito, hash do QoS efetivo e motivos de recusa/fallback. Não prometer Reliability/History distintos por amostra. Ownership strength continua por papel, jamais por confiança/prioridade Laya.

Domínio adaptativo dedicado e engine sem dependência de orchestrator/qos-nfcm eliminam a presença do controlador legado nesse caminho. Falhar no gate de implantação se um componente legado estiver configurado para esse domínio. Detectar configuração não constitui defesa contra participantes não autenticados.

## Alternativas

- Endpoints pré-criados por perfil: possível evolução, adiada porque requer matching de todos os readers, partições/tópicos, limites e observação de ordering/ownership; não necessária para validar classificação shadow.
- Mutar writer compartilhado a cada pedido: rejeitado por interferência entre workflows.
- Reaproveitar NFCM como fallback: rejeitado por D4/D5.

## Gate e rollback

V07 verifica duas requisições simultâneas com perfis distintos, atributos DDS invariantes, prioridades de aplicação observáveis e recusa de estrutura incompatível. Se QoS nativo variável for requisito de aceite do usuário, esta proposta não o satisfaz: registrar decisão D-QOS em risks e especificar endpoints antes de M4. Rollback desabilita admissão adaptativa; nenhum knob do domínio legado é modificado.
