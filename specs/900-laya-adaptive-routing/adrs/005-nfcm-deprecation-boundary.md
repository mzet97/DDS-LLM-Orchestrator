# ADR-005 — Aposentadoria NFCM sem perda acadêmica

Data: 2026-10-16. Status: aposentadoria para evolução DECIDIDA PELO USUÁRIO; roteiro de isolamento PROPOSTO.

## Decisão

REQ-900-005/012/016. Nada novo em qos-nfcm; novo caminho não importa seus algoritmos, tipos ou fallback. Preservar runtime e testes legados durante M0–M6. Remoção/isolamento só em M7, após inventário de dependentes, compatibilidade e regressões. Código ainda executado não torna vigente a estratégia aposentada; torna a migração necessária.

## Evidência e impacto

`cargo tree -i qos-nfcm` mostra orchestrator e benchmarks. O modo static e os testes de reaper/monitor também usam tipos da crate. Flags, StabilityController, perfis globais e métricas históricas formam dependências distintas. Inventário completo em [audit](../audit.md), seção NFCM.

Antes de retirar dependência do runtime, separar os tipos neutros necessários ou criar adapters de compatibilidade restritos ao legado; a mudança precisa ter sua própria task/revisão. Manter branch/tag de congelamento, prompts, scripts e resultados do artigo/campanhas. Não remover o braço fuzzy dos benchmarks para fazer a árvore de dependências parecer limpa.

## Alternativas rejeitadas e gate

Remover crate agora quebra testes/baselines; atualizar apenas README deixa loop ativo; usar NFCM no fallback contradiz o usuário. V12 compara inventário de dependentes antes/depois, CLI/config, testes históricos e hashes protegidos. Rollback restaura a configuração legada validada sem alterar resultados acadêmicos. M0 não realiza essa remoção nem muda defaults.
