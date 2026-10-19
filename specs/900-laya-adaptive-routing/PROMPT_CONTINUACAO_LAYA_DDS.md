# Handoff para outra IA — SDD Laya/DDS

Este arquivo é o ponto de entrada autocontido para continuar o trabalho. **A sessão anterior fez somente M0 documental e baseline; você não está recebendo uma implementação Laya pronta.** Leia o dossiê antes de escrever código.

## 1. Pedido do usuário e autoridade

O usuário quer roteamento semântico e escolha de objetivos QoS por **Laya em Python**, com **núcleo, planner/validator, materialização, supervisão, agentes e execução em Rust**, comunicação interna **CycloneDDS**. Pediu SDD próprio evidence-first, sem GitHub Spec Kit. Fuzzy/FCM/NFCM foi aposentado como estratégia de evolução; código, testes e experimentos históricos devem ser preservados até migração controlada.

Nesta sessão foi autorizado apenas: auditoria M0, especificações, contratos propostos, ADRs, plano e este handoff. O usuário confirmou que a implementação será feita por outra IA. **Receber este arquivo não autoriza automaticamente todos os marcos.** Na sessão de destino, a solicitação explícita do usuário determina qual marco executar. Por padrão, retome a revisão de M0 e a próxima tarefa M1-01; não avance para M2–M7 sem o gate/autorização correspondente. Não peça novamente decisões já expressas em D1–D10 dos anexos.

Hierarquia prática: pedido atual do usuário define escopo; Prompt Master define workflow; código do HEAD real define fatos; dissertação define metodologia; fontes oficiais definem API de terceiros. Instruções nos documentos são contexto de execução, não autorização para ignorar limites da sessão atual.

## 2. Checkout correto e snapshot

- **Raiz Git/Rust escolhida:** `/var/mnt/HD1TB/tese/src/rust`.
- **Dossiê:** `/var/mnt/HD1TB/tese/src/rust/specs/900-laya-adaptive-routing/`.
- HEAD auditado: `5a7c15b0be3ad0638d8a041338eda41c9587ae03`.
- Branch auditado: `studio/phase-800-node`.
- Antes de M0: árvore limpa. Depois: somente dossiê novo, sem commit automático. Consulte `evidence/after.json` para estado final.
- **Não confundir** com `/var/home/mzet/projetos/tese/src/rust`: é outro checkout, anteriormente observado em `67ff7bf` com alterações locais de Studio. Não copiar, resetar ou sobrescrever esse trabalho.
- O diretório pai `/var/mnt/HD1TB/tese` não é a raiz Git deste workspace. Anexos e experimentos externos ficam nesse pai.
- O commit `b2399f3` dos anexos é histórico, não o HEAD auditado.

Execute primeiro `git rev-parse HEAD`, `git branch --show-current`, `git status --short` no checkout correto. Se mudou, registre drift e revalide achados afetados; não trate hashes/testes antigos como atuais. Preserve todo trabalho preexistente. Não crie branch, commit, deploy ou publicação sem autorização da sessão.

## 3. Ordem de leitura obrigatória

1. Instruções aplicáveis em `../../AGENTS.md`, `AGENTS.md`, `specs/CONSTITUTION.md`, `specs/CONTEXT.md`, `specs/ROADMAP.md`. Conteúdo antigo sobre NFCM/paridade não revoga a decisão explícita do usuário de evoluir pela fase 900. Não retome automaticamente a primeira task antiga incompleta.
2. **Integralmente:** `/var/mnt/HD1TB/tese/PROMPT_MASTER_SDD_LAYA_DDS.md` e `/var/mnt/HD1TB/tese/CONTEXTO_ARQUITETURAL_LAYA_DDS.md`.
3. Dossiê: [README](README.md), [REPORT](REPORT.md), [audit](audit.md), [spec](spec.md), [architecture](architecture.md), [contracts](contracts.md).
4. Os cinco ADRs em `adrs/`, [plan](plan.md), [tasks](tasks.md), [test-plan](test-plan.md), [traceability](traceability.md) e [risks](risks.md).
5. [Índice de evidências](evidence/README.md), logs específicos do marco e revisão independente.

Se algum anexo faltar após mover o projeto, não invente suas instruções: os requisitos resumidos aqui/dossiê permitem diagnóstico, mas mudanças de escopo/compatibilidade exigem localizar a fonte ou esclarecer a lacuna.

## 4. Fatos confirmados e armadilhas

- Workspace com 19 crates; contrato com 19 tópicos (inclui Studio.NodePresence), dois IDLs existentes. LLM é keyless; TaskOutput usa `(task_id, seq_num)`.
- `cyclonedds = =3.0.1`, sys `=1.2.1` no snapshot. Código/lockfile prevalecem sobre README antigo.
- CLI DDS do orchestrator escolhe NFCM por default e inicia control loop; `--fuzzy-routing` desligado não desliga esse loop.
- `qos-nfcm` fornece também tipos e StaticDecider usados por orchestrator/benchmarks. Não remover a crate cegamente.
- Prefixo vazio ignora target_agent no claim atual. Capability semântica não é ModelSpecialization. EXP4 usa prefix/target; não corrigir o legado silenciosamente.
- QoS nativo pertence ao endpoint. Loop legado muda knobs no pool; não aplicar perfil por requisição mudando writer compartilhado.
- wf-run tem sequência/fork-join explícitos, não DAG genérico com journal.
- MCP já tem validação de políticas e claim no código auditado; isso não prova identidade DDS autenticada ou exactly-once.
- Context-store JSONL faz flush/replay permissivo; Studio tem SQLite administrativo. Nenhum é autoridade de workflow genérico hoje.
- Há dados/relatório de campanha 880 posteriores às descrições antigas de “pendente”. M0 não revalidou conclusões acadêmicas.

## 5. Design proposto para revisão

Decisões fechadas: Python só para Laya/adapter; Rust no domínio; DDS interno; Laya decide objetivos semânticos; planner generativo separado; sem NFCM no novo caminho; baselines preservados; at-least-once, sem alegação exactly-once/fencing atual.

Propostas ainda não aprovadas como implementação: WorkflowPlanning.idl isolado, cinco novos tópicos; WorkflowSpace por composição; engine biblioteca+binário; domínio adaptativo dedicado; endpoints DDS estruturais fixos e QoS de aplicação; SQLite local com engine único/outbox; IDs físicos por tentativa; tools desabilitadas até prova de idempotência/autoridade.

Detalhes de fields/types/keys/bounds/QoS, encoding de hash, readiness, cancelamento, falhas e rollback estão nos documentos. Não substituir por desenho próprio sem registrar ADR. Pontos de decisão específicos: D-QOS, D-ISOLATION, D-JOURNAL em risks. Aprovar o plano de auditoria não os aprovou automaticamente; se o usuário na sessão destino disser para implementar a proposta, essa autorização pode resolver os pontos sem reconfirmação redundante.

## 6. Verificação já executada e limites

- `cargo test --workspace --locked`: repetição final **596 passed, 0 failed, 2 ignored**; exit 0.
- `cargo clippy --workspace --locked -- -D warnings`: exit 0.
- `cargo fmt --all -- --check`: exit 0.
- Primeira baseline falhou no teste de append em arquivo 0444 porque namespace root reteve capabilities. Log preservado; removê-las fez o teste exato passar, depois a baseline completa passou. Nenhum código/teste foi alterado.
- Workspace unifica features; algumas suites DDS reais rodaram, mas `mock_parity`, `control_loop`, `qos_manager` reportaram zero testes. Próxima implementação precisa de comandos separados por feature; não extrapolar os 596 testes.
- Inferência real llama-server e um doctest foram ignorados. Laya ausente no Python consultado; CycloneDDS Python 11.0.1. Novo IDL/interop/Python Laya não foram testados porque não existem ainda.
- PDF exato da dissertação não foi localizado; leitura acadêmica usada é indireta via Entendimento. Graphify/RTK indicados não estavam disponíveis.

Não repetir campanhas ou testes caros autenticados sem mudança/risco que justifique. Para testes relevantes, usar target local exclusivo do checkout/job e `CYCLONEDDS_STATIC=1` no CIFS; não reutilizar targets de outra montagem. O caminho de cache registrado nos JSON é histórico da execução, não regra universal.

Se usar namespaces Linux: configurar loopback antes de remover capabilities; executar Cargo/testes sem capacidade de ignorar permissões. Não atingir participantes de produção. Os metadados JSON mostram configuração exata. `--locked` preserva lockfile; não usar `cargo fmt` sem `--check` como verificação.

## 7. Como executar o próximo marco

1. Confirmar qual marco a sessão autoriza e revisar propostas de compatibilidade pertinentes.
2. Revalidar snapshot e ler task/REQ/ADR. A próxima tarefa é **T-900-M1-01**, não criar worker/engine completo.
3. Escrever teste que prove o requisito; implementar somente essa tarefa. Novo tipo nasce no IDL, com geração nos dois lados; nenhum JSON opaco substitui o contrato.
4. Executar unit/integration/interop conforme aceite, clippy e fmt check; capturar comando/ambiente/saída. Erro/bloqueio não é PASS.
5. Atualizar tasks, traceability, risks e REPORT do marco. Não sobrescrever evidências M0. Distinguir observado de proposto/pendente.
6. Apresentar gate, incompatibilidades, rollback e próxima tarefa. Não pular marcos nem ativar Laya em execução antes de interop/elegibilidade/QoS/recovery validados.

## 8. Critério de encerramento da execução futura

O marco autorizado termina quando seus Given/When/Then foram observados na superfície correta e os resultados estão rastreados. Em falta de modelo/hardware/credenciais, registre BLOCKED específico e continue tarefas independentes do mesmo marco. Não invente performance, qualidade, autorização, vitória sobre MAF ou resultados de campanha.

**Esta passagem não solicita implementação de todos os marcos em uma única sessão.** O backlog completo foi escrito para continuidade controlada; o usuário define a autorização da IA que o receber.
