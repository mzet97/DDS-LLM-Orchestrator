# Plano de execução e migração

Status: M0 autorizado; M1–M7 somente planejamento. Diagnóstico em [audit](audit.md), requisitos em [spec](spec.md), ledger em [tasks](tasks.md).

## M0 — entrega documental

1. Revalidar HEAD/branch/status, ferramentas, hashes e corpus protegido.
2. Inspecionar manifestos/IDL/codegen, transporte, claim, control loops, drivers, segurança, journals e consumidores NFCM; confrontar anexos/documentos acadêmicos.
3. Inspecionar efeitos dos testes, executar baseline com lockfile/target isolado e DDS restrito a namespace de rede; registrar PASS/FAIL/BLOCKED sem corrigir runtime.
4. Produzir requisitos, arquitetura, contratos propostos, cinco ADRs, testes, riscos e rastreabilidade.
5. Revisar referências, consistência e preservação; apresentar REPORT e decisões de compatibilidade. Não criar crate, worker, IDL novo, teste funcional, branch ou commit.

## Sequência posterior — execução não autorizada

| Marco | Trabalho atômico inicial e dependências | Gate e rollback |
|---|---|---|
| M1 | Após revisão M0: congelar fixtures de tipo/hash e introduzir IDL isolado + codegen/trait composta/mock. Não materializar Task. | Rust↔Python real, keys/typename/CDR/XTypes/bounds, late joiner, mismatch, versão; desligar novos endpoints sem tocar os existentes. |
| M2 | Fixar SDK/licenças/checkpoint/perguntas/dataset; adapter Python apenas shadow com erros/timeout. | Sem writes Tasks, métricas de qualidade/calibração e latência; desligar worker. |
| M3 | Introduzir catálogo e caminho de claim adaptativo no domínio dedicado, sem mudar prefix matching legado. | Elegibilidade/barreira/health/slots, K=2/K=4 e regressões; retirar admissão e drenar/cancelar. |
| M4 | Mapper Rust de decisão validada para objetivos de aplicação, com QoS efetivo observado. | Concorrência entre perfis sem mutação global; estrutura incompatível recusada; desabilitar mapper adaptativo. |
| M5 | Engine Rust, planner adapter, validator, journal/outbox, materializer, replay/cancelamento. | DAG e recovery/in-doubt em falhas reais; conservar journal e suspender admissão. |
| M6 | Canary e estudo complementar após todos os gates aplicáveis; fixtures/prompts próprios versionados. | Manifests, comparação controlada, análise e ameaças; voltar ingresso ao modo anterior sem migrar tasks no meio da execução. |
| M7 | Retirada/isolamento controlado NFCM conforme ADR-005. | Dependentes, CLI, regressões e preservação acadêmica; rollback de configuração/código específico aprovado. |

M3 pode testar agentes com fixtures sem engine final; M4 pode testar mapper sem canary. Ativação de execução adaptativa real exige M1–M5 e revisão, não apenas existência de flag. A cronologia M4/M5 pode ser reordenada com atualização do plano; gates não são dispensados.

## Controles transversais

- Snapshot de cada marco com SHA, lockfiles, hashes dos schemas/configs/modelos; não reutilizar aprovação de M0 para mudança funcional.
- Baselines legados mantidos; estudo Laya separado, sem reclassificar resultados antigos nem adicionar RQ5 à dissertação automaticamente.
- Não baixar pesos em teste unitário; integração real opt-in. Latência/acurácia alvo depende de baseline/piloto e deve ser congelada antes da campanha.
- Nenhuma decisão de QoS ou Laya pode ampliar segurança/capabilities. Falha desconhecida retorna causa tipada, não fallback silencioso.
- Rulings do executor e bloqueios estão em tasks/risks; atualizações de documentos antigos fora desta pasta serão propostas separadamente, evitando modificar a fonte histórica durante M0.
