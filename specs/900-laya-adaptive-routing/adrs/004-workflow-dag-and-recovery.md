# ADR-004 — DAG, autoridade e recuperação

Data: 2026-10-16. Status: PROPOSTO; nenhuma tabela, crate ou migração criada.

## Contexto

REQ-900-008/009/010/011/015. wf-run é driver explícito de sequência/fork-join. Context-store possui journal leve sem transação de plano/outbox e replay permissivo; Studio possui armazenamento administrativo SQLite. Nenhum equivale a engine persistente de DAG. Ver audit A10/A12/A14.

## Decisão proposta

Engine único por domínio, biblioteca Rust pura e binário opt-in. Planejamento candidato via inferência DDS existente; validação completa antes de publicar plano. Persistência SQLite local dedicada ao workflow, reutilizando rusqlite e padrões existentes sem depender do Studio. Journal é autoridade; Workflow.Plan/State são projeções.

Transação inclui decisão aceita, plano/nós/tentativa e outbox. Worker duplicado pode produzir múltiplas decisões; primeiro commit válido para as revisões congeladas vence e fica persistido. Isso não é eleição distribuída nem escolha independente da ordem de chegada. Uma implantação com dois engines é rejeitada pela política V1; lock de arquivo impede dois processos locais, não dois hosts.

IDs físicos incluem attempt_id e mantêm identidade lógica por nó. Estado controla tentativa ativa e filtra saídas antigas. Falha entre efeito externo e persistência produz IN_DOUBT se o handler não garantir idempotência; reexecução automática é proibida nesse caso. Cancelamento é cooperativo e não desfaz efeito já realizado.

## Alternativas rejeitadas

- DDS TransientLocal como única persistência: não cobre perda de todos os writers.
- Usar Context.Snapshot como segundo banco de estado de workflow: cria autoridade ambígua e não oferece transação/outbox.
- Banco administrativo do Studio: acoplamento de ciclo de vida e permissões indevido.
- SQLite WAL em CIFS ou múltiplos engines sem coordenação: fora das garantias propostas.

## Gate

V08/V09/V10: ciclo, limite, dependência inválida; restart antes/depois do commit/publicação/ASSIGNED/RUNNING/side effect/chunk; duplicatas; saída final e DONE em ordem invertida; cancelamento e journal corrompido. Recuperação falha fechada em corrupção, não ignora registros. Rollback desliga admissão, preserva DB/outbox para reconciliação e não apaga dados para aparentar sucesso.
