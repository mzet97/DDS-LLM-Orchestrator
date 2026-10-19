# ADR-003 — Capabilities, barreira de elegibilidade e claim

Data: 2026-10-16. Status: PROPOSTO; sem alteração ao claim atual em M0.

## Contexto

REQ-900-006/007/008/011/012. Prefixo vazio aceita target de qualquer agente no comportamento atual. EXP4 usa target/prefixo explicitamente. Anúncios em tópicos separados não são publicação atômica. Evidências A03/A04/A09/A10 do [audit](../audit.md).

## Decisão proposta

Catálogo semântico versionado separado de ModelSpecialization. Agentes adaptativos exigem capabilities autorizadas e frescas, registry compatível, saúde/slots/modelo/política e target exato quando preenchido. Validar na ingestão, antes do claim e após readback antes da execução.

Sem ampliar Task V4: agente cruza TaskId com plano imutável e tentativa ativa em Workflow.State; sem ambos, espera/reavalia até timeout. Um campo ready isolado não basta. Estado e plano podem chegar antes ou depois da Task. Testar rediscovery/replay e ausência de snapshot.

Agentes adaptativos operam em domínio dedicado. O modo legado mantém prefix matching original e seus experimentos; a nova regra não corrige silenciosamente EXP4. Catálogo anunciado não autentica agente; autorização externa/política continuam obrigatórias.

## Alternativas rejeitadas

- Reutilizar especialização TEXT como security_audit: mistura capacidade e modelo.
- Só publicar requirements antes da Task: ordering inter-tópicos não provado.
- Misturar agentes antigos no domínio adaptativo: eles não aplicam a barreira e podem executar com prefixo vazio.
- Dispatcher central escolhendo vencedor do claim: muda semântica data-centric sem necessidade.

## Prova e rollback

V05/V06/V09: ausência/falsificação/expiração de capability, target errado com prefixo vazio, ordem invertida de tópicos, contenção K=2/K=4, agente lento/morto, modelo ausente e regressões EXP1b/EXP4. Rollback impede novas intenções, drena/cancela workflows e desliga domínio adaptativo; não reenviar suas tasks ao legado.
