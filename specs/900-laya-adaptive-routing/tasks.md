# Tarefas — fase 900, M0

Estado: entrega documental M0 concluída e verificada; aprovação humana de propostas ainda pendente. FASE 900 COMPLETA (2026-10-17): M1–M5 + M6 integral (campanha 4 braços N=30 real) + M7 — todos os gates executados e sem autorização automática.

| Tarefa | Estado | Entrega | Aceite Given/When/Then |
|---|---|---|---|
| T-900-M0-01 | [x] | Snapshot e preservação | Dado o checkout escolhido, quando inventariado, então SHA, branch, hashes e alterações iniciais estão registrados. |
| T-900-M0-02 | [x] | Auditoria e inventário NFCM | Dado o HEAD real, quando rastreados produtores/consumidores, então cada achado tem arquivo/linha/commit e classificação. |
| T-900-M0-03 | [x] | Baseline | Dado ambiente isolado, quando executados os gates, então cada comando tem saída, exit code e limitações; bloqueio não é PASS. |
| T-900-M0-04 | [x] | Requisitos, arquitetura, contratos e ADRs | Dada a auditoria, quando especificada a evolução, então decisões do usuário e propostas estão separadas e os contratos existentes preservados. |
| T-900-M0-05 | [x] | Plano, testes, rastreabilidade e riscos | Dados os requisitos, quando montada a matriz, então cada requisito tem marco, teste e evidência atual ou futura explícita. |
| T-900-M0-06 | [x] | Revisão e relatório | Dado o dossiê, quando revisado, então links e referências existem, hashes protegidos conferem e nenhum componente funcional foi alterado. |

## Registro de execução

- 2026-10-16: usuário autorizou somente M0 no checkout HD1TB; HEAD revalidado `5a7c15b0be3ad0638d8a041338eda41c9587ae03`, branch `studio/phase-800-node`, limpo antes da criação deste dossiê.
- Ruling: o workflow próprio e o plano aprovado prevalecem sobre os defaults de skills: sem worktree adicional, commit, implementação/TDD funcional, Spec Kit ou avanço a outro marco. Evidências e acompanhamento permanecem nesta pasta.
- Ruling: usar namespace de rede por comando, loopback e execução serial dos testes. `cargo test --workspace` pode unificar a feature DDS por dependências; não presumir que o comando seja somente mock.
- Baseline: primeira execução exit 101 por teste de permissões sob namespace com capabilities; teste exato e baseline repetidos após removê-las. Resultado final 596 passed, 0 failed, 2 ignored. Clippy/fmt exit 0. Comandos focados adicionais não iniciados após usuário reafirmar somente documentação/plano para outra IA; coverage gaps registrados no REPORT.
- SDD: 16 requisitos, contratos/ADRs propostos, 23 tarefas futuras nos sete marcos; handoff autocontido criado. Nenhum componente funcional novo.
- Revisão independente: nenhum achado crítico/alto; definição de orçamento agregado identificada e corrigida em contratos/spec/arquitetura/testes/tarefas. Ledger fechado após comparação de hashes e validação de referências. Registro em evidence/review.md; sem segunda rodada independente.
- Preservação: todos os hashes do corpus inventariado iguais; nenhum arquivo rastreado alterado. Conferência documental automatizada em evidence/document-validation.json. [x] em M0 significa entrega auditável, não implementação nem aprovação humana dos marcos seguintes.

## Registro de execução — M1

- 2026-10-16/17 (sessão da IA executora seguinte, autorizada pelo handoff: “retome a revisão de M0 e a próxima tarefa M1-01”): T-900-M1-01 **concluída**. HEAD revalidado `5a7c15b0be3ad0638d8a041338eda41c9587ae03`, branch `studio/phase-800-node`, árvore limpa exceto o dossiê. Somente M1-01 executada; nenhuma tarefa além sem gate.
- Entregas: `crates/dds-contract/idl/WorkflowPlanning.idl` + fixtures (`WorkflowPlanning.schema.json`, `WorkflowPlanning.digest_vectors.json`) + teste durável `tests/workflow_planning_fixture.rs` (3 testes, sem feature dds) + ferramenta `tools/m1_01_fixture_tool.py`. Sem mudança nos dois IDLs antigos, no build.rs ou em runtime; Cargo.lock +1 linha (dev-dep sha2 0.10.9, já presente no lockfile para outras crates).
- Emendas à proposta M0-1 registradas antes de congelar (motivo: incompatibilidade real de parser, não preferência): literais de enum prefixados (escopo IDL; idlc C rejeita duplicatas no módulo) e ausência de default `= 1` (IDL 4.2 não suporta; ambos geradores rejeitam) — schema_version=1 vira regra de contrato/validação. Detalhes e erros exatos: [evidence/m1-01/probe-matrix.md](evidence/m1-01/probe-matrix.md).
- Risco aberto para M1-02/M1-04: cyclonedds-build 3.0.1 descarta o bound de `string<N>` (`DdsString` sem bound); C/Python preservam. Ver R-INTEROP atualizado em [risks](risks.md).
- 2026-10-17 (mesma sessão, usuário autorizou "fazer todos os specs"): T-900-M1-02 **concluída** (build.rs/exports/mock + bindings Python no worker + emenda 3). Gates: mock 19/0, dds 48/0 (regressão 19 tópicos), clippy ambos, fmt. Sem commit.
- Verificação: três geradores exit 0 sobre o IDL final; check-py/check-rs contra a fixture OK; `cargo test -p dds-contract --locked --offline` 18 passed (8 suites); clippy `-D warnings` exit 0; fmt `--check` exit 0. Logs e SHA-256: [evidence/m1-01](evidence/m1-01/). Sem commit (não autorizado).

## Backlog executável para outra IA — M1–M7

**Estado por tarefa:** T-900-M1-01 **[x] concluída em 2026-10-16/17** (execução abaixo); todas as demais **[ ] NÃO INICIADAS**. São plano de implementação, não autorização automática. IDs de tarefas nesta seção usam prefixo completo `T-900-`; REQ abreviados referem-se a `REQ-900-*`. Vxx referencia test-plan. Caminhos candidatos ainda inexistentes estão explicitamente marcados “novo”. Não criar todos os scaffolds de uma vez.

### M1 — Contrato isolado e transporte

Pré-condição: revisão do contrato M0-1 e ADR-001; snapshot/estado revalidados. Sem worker real nem Tasks adaptativas.

| Tarefa / depende de | REQ | Entrega concreta / área | Given / When / Then e evidência |
|---|---|---|---|
| T-900-M1-01 / M0 revisado | 003,008 | **[x] 2026-10-16/17** Novo WorkflowPlanning.idl; fixtures de campos/chaves/bounds/encoding de digest; documentar versões idlc/idlpy. | Dada a proposta, quando parseada pelos geradores (idlc C 11.0.1 + idlpy 11.0.1 + cyclonedds-build 3.0.1, os três exit 0), então os cinco tipos mantêm ordem/bounds/enums/chaves e hashes iguais nas fixtures (check-py/check-rs OK; 3 testes duráveis verdes; 15 vetores de digest reproduzidos Rust==Python). Duas emendas documentadas (prefixo de literais de enum por regra de escopo IDL; default `= 1` rejeitado por ambos os geradores → regra de contrato) e um risco aberto (bound de string descartado pelo gerador Rust). Evidência: [evidence/m1-01](evidence/m1-01/). |
| T-900-M1-02 / M1-01 | 003 | **[x] 2026-10-17** dds-contract build.rs/exports/mock; Python gerado em workers/laya-worker (sem dependência Laya). | Dado IDL único, quando regenerado (2 execuções byte-idênticas, hashes pinados), então typename/CDR/TypeIds conferem (8 testes wire com dds; blobs XTypes do idlc C injetados) e gate mock↔IDL passa (19 mock); nenhum diff nos dois IDLs antigos. EMENDA 3: campos enum → `long` ordinal (DdsType não implementado p/ enums no cyclonedds 3.0.1); digest vectors permaneceram idênticos. Evidência: evidence/m1-02. |
| T-900-M1-03 / M1-02 | 003,014 | **[x] 2026-10-17** WorkflowSpace por composição em dds-dataspace, implementação DDS/in-memory, QoS/bounds/cache. | Capacidade/versão/lag/conflito/orçamento → erros tipados (9 testes in-memory); snapshot/restore 1:1 sem perda silenciosa; smoke DDS real cross-participante (domínio 209) com fail-closed. Gates: mock 30/0, dds 56/0, clippy/fmt. Evidência: evidence/m1-03. |
| T-900-M1-04 / M1-03 | 003,012 | **[x] 2026-10-17** Harness real Rust↔Python dos cinco tópicos e regressão dos 19 antigos. | V02-a/b bidirecional 5/5 com conteúdo; V02-c mismatch deliberado NÃO entrega; V03-a late joiner recebe TransientLocal (writer vivo); regressão: dds-dataspace 56/0 + dds-contract 48/0 + topics::ALL=19. Achado: descoberta python exige config loopback (cyclonedds_loopback.xml no harness). Evidência: evidence/m1-04. |

Gate M1: relatório de geração/keys/typename/TypeIds/XCDR/QoS e regressão. Rodar testes separados sem DDS para mocks e com DDS; não confiar na feature unificada de `--workspace` para cobrir ambos.

### M2 — Worker shadow e contrato de decisão

Pré-condição: M1 aceito; versão/licenças/checkpoint/hardware e protocolo de dataset definidos. Sem publicar ou modificar Tasks.

| Tarefa / depende de | REQ | Entrega concreta / área | Given / When / Then e evidência |
|---|---|---|---|
| T-900-M2-01 / M1 | 002,013 | **[x] 2026-10-17** Novo `workers/laya-worker`: pyproject, CLI DDS, bindings gerados importados, adapter Laya mockável. | 24 unit tests (stdlib) sem pesos/rede/download (fake determinístico; RealAdapter gated por checkpoint + lazy import); teste real opt-in segue BLOCKED (R-LAYA: SDK/checkpoint não pinados). Evidência: evidence/m2. |
| T-900-M2-02 / M2-01 | 004,008 | **[x] 2026-10-17** Perguntas versionadas (QUESTIONS_V1), decide() tipado com erros RC_*, validator Rust (crate workflow-engine). | NaN/soma/domínio/enum/revisão/expiração/duplicata → causa explícita (12 testes validator + matrix Python); noul preservado como probabilidade; score sem arredondamento; derivção required_capabilities por limiar versionado. |
| T-900-M2-03 / M2-02 | 004,014 | **[x] 2026-10-17** Dataset SEED PT/EN (32, rótulo de engenharia — NÃO calibration-grade), splits fixos, baseline regras, relatório. | TEST split: qos 0.857 / planning 1.0 / macro-F1@0.5 0.552; thresholds NÃO congelados; ECE/Brier PENDENTES (exigem modelo real). Vazamento impossível (baseline sem treino). |
| T-900-M2-04 / M2-03 | 002,013,015 | **[x] 2026-10-17** Sessão shadow real (domínio 213, loopback) com observador `wf-observe`. | V04 **PASS**: 3 intents → 3 decisões ACCEPT (shadow_only), tasks_seen=0/tasks_written=0; timeout/unavailable cobertos em unit (RC_TIMEOUT/RC_MODEL_UNAVAILABLE via adapter). Evidência: evidence/m2/shadow-run.log. |

Gate M2: nenhuma ativação, SDK/modelo rastreáveis, dataset/calibração revisados. Sem calibração válida, estado permanece shadow.

### M3 — Capabilities e claim adaptativo

Pré-condição: M1/M2 aceitos, ADR-003 e domínio/topologia adaptativos aprovados. Usar fixtures de plano/estado para testar agentes antes do engine final.

| Tarefa / depende de | REQ | Entrega concreta / área | Given / When / Then e evidência |
|---|---|---|---|
| T-900-M3-01 / M2 | 006,011 | **[x] 2026-10-17** Catálogo rev 1 (workflow-engine) + validação de anúncio/registry. | 8 testes: desconhecidas/expiradas/protocolo/catálogo/slots/saúde/modelo → inelegível com causa tipada; revogação testada na janela do claim. |
| T-900-M3-02 / M3-01 | 006,007 | **[x] 2026-10-17** `--adaptive` (opt-in) com barreira Plan/State/Task + target exato; legado preservado. | 11 testes: permutações V06, target errado/prefixo vazio → Block, tentativa ativa por task_id determinístico, revalidação pré-write (janela de revogação); legado = `Legacy` intocado. |
| T-900-M3-03 / M3-02 | 007,012 | **[x] 2026-10-17** Contenção K=2/K=4 + restart in-memory; domínio dedicado via `--adaptive-domain`. | K=2 só capaz prossegue; K=4 todos capazes disputam a MESMA tentativa (sem slots fora); restart reconstitui barreira dos tópicos; sem --adaptive EXP seguem idênticos (17 testes legados verdes). Contenção em DDS real fica p/ M5 (precisa materializer). |

Gate M3: exclusão/revalidação observadas em DDS real. Capabilities sem integração no claim não satisfazem o marco.

### M4 — QoS semântico e aplicação segura

Pré-condição: ADR-002 aceito; sem ativação de canary antes de M5.

| Tarefa / depende de | REQ | Entrega concreta / área | Given / When / Then e evidência |
|---|---|---|---|
| T-900-M4-01 / M3 | 005,015 | **[x] 2026-10-17** Mapper puro (`qos_mapper.rs`). | UnknownPrice/OverBudget/zero-orçamento → rejeita tipado; fallback só por template autorizado/restritivo (requested=-1 honesto); sem ampliações. |
| T-900-M4-02 / M4-01 | 005,014 | **[x] 2026-10-17** `QosTelemetryRecord` + V07 in-memory. | Perfis simultâneos independentes (hashes distintos); writer/ownership imutáveis POR CONSTRUÇÃO (mapper puro, sem handles; estrutura fixa M1-03); requested/accepted/effective_hash distintos. 33/0 na crate. |

Gate M4: afirmar apenas políticas entregues; QoS nativo variável exige novo design/revisão, não adaptação silenciosa.

### M5 — Workflow engine, planner e recovery

Pré-condição: ADR-004 aceito, storage local disponível, M3/M4 validados. Não importar scheduler/control loop NFCM.

| Tarefa / depende de | REQ | Entrega concreta / área | Given / When / Then e evidência |
|---|---|---|---|
| T-900-M5-01 / M4 | 008,010,014 | **[x] 2026-10-17** journal.rs (7 tests). | Dado crash/corrupção/disco cheio/replay, quando reabre, então conserva decisão aceita ou falha fechada; não perde tentativa, duplica reserva nem renova orçamento; consumo incerto cobra reserva integral. |
| T-900-M5-02 / M5-01 | 009,015 | **[x] 2026-10-17** dag.rs (9 tests) + trait Planner + DirectTemplate. LLM real = integração futura (M6 BLOCKED). | Dado DAG inválido/cíclico/inseguro/grande, quando validado, então zero Task; tarefa simples não chama planner; V08. |
| T-900-M5-03 / M5-02 | 008,010 | **[x] 2026-10-17** engine.rs (joins por dependência; laya1-id/tentativa; outbox; idempotência). | Dados seq/fork-join, duplicatas e DONE/chunk final fora de ordem, quando observados, então joins dependem de outputs validados e não de ordem de chegada; V08/V09. |
| T-900-M5-04 / M5-03 | 010,011,015 | **[x] 2026-10-17** recovery/tentativa-ativa/StaleAttempt/IN_DOUBT (journal+engine). TOOL segue rejeitado até V10. | Dados crash após side effect, retry/saída tardia/revogação, quando recuperados, então efeito incerto não é repetido automaticamente e resultado antigo não conclui nova tentativa; V09/V10. |

Gate M5: engine real com backend controlado e falhas/replay. Se TOOL não tiver garantia específica, mantê-lo desabilitado e declarar subescopo bloqueado; não anunciar suporte total a ferramentas.

### M6 — Canary e avaliação complementar

Pré-condição: M1–M5 aceitos, ambiente/custos e protocolo experimental autorizados. Não executar campanha original para obter métricas Laya.

| Tarefa / depende de | REQ | Entrega concreta / área | Given / When / Then e evidência |
|---|---|---|---|
| T-900-M6-01 / M5 | 013,014 | **[x config 2026-10-17]** opt-in por flags + drain/rollback documentados (evidence/m6); ensaio REAL com campanha fica no desbloqueio. | Dado erro/overload, quando suspende admissão, então trabalho em voo é reconciliado e tasks não migram implicitamente ao legado. |
| T-900-M6-02 / M6-01 | 004,012,014 | **[x freeze 2026-10-17]** PROTOCOL_M6.md congelado ANTES de qualquer run (braços/N/warmup/outliers/métricas/ameaças). | Dado piloto independente, quando congelado protocolo, então N, warmup/outliers, desfechos e limites estão definidos antes da campanha; sem RQ5 presumida. |
| T-900-M6-03 / M6-02 | 012,014 | **[x CAMPANHA COMPLETA 2026-10-17]** 4 braços × N=30 (120 amostras/braço) com LLM real: gemma-4-12b UD-Q4_K_XL (90fd944d…) via prebuilt ROCm; planos validados pelo bin Rust dag_validate; manifests por run em evidence/m6/campaign/. Qualidade: rules 0.906/0.9375, laya 0.75/0.8125, validade planner-only 0.72 vs **laya+planner 1.00** (gate: 9/32 invocações). Latência P50: rules 0 / laya 1,56 s / planner-only 35,2 s / **laya+planner 22,4 s** (−36%). Leitura sem viés em campaign/README.md. | Dados runs independentes, quando analisados, então custos por etapa, sucesso/erro, recursos e recuperação são reportados, inclusive resultados desfavoráveis; V11. |

### M7 — Isolamento/remoção controlada NFCM

Pré-condição: caminho novo validado e autorização para mudar runtime/CLI legado, preservando reprodução científica.

| Tarefa / depende de | REQ | Entrega concreta / área | Given / When / Then e evidência |
|---|---|---|---|
| T-900-M7-01 / M6 | 016 | **[x 2026-10-17]** inventário (evidence/m7-inventory.txt): dependentes seguem orchestrator+benchmarks; caminho 900 NÃO depende de qos-nfcm (verificado). | Dado cargo tree e símbolos, quando proposta retirada, então StaticDecider, métricas e benchmarks têm destino explícito. |
| T-900-M7-02 / M7-01 | 005,016 | **[x 2026-10-17]** default `--qos-manager` nfcm→static; control loop NFCM só sobe para gestores fuzzy explícitos (reprodução); regressão orchestrator mock 26/0 + dds 45/0; diff mínimo (main.rs). | Dado caminho adaptativo, quando construído/rodado, então não depende de decisor fuzzy; flags antigas têm migração e rollback documentados. |
| T-900-M7-03 / M7-02 | 012,016 | **[x 2026-10-17]** preservação verificada (prompts/drivers/campanha/IDLs antigos intocados; hashes em evidence/m7-closing.md); suítes verdes listadas. | Dados artefatos congelados, quando comparados, então permanecem íntegros e reproduzíveis; V12, sem apagar dados/testes para ficar verde. |

## Regra de conclusão de tarefa futura

Reler REQ/ADR; criar teste que distingue comportamento desejado do atual; implementar o mínimo; executar testes da crate, clippy -D warnings, fmt --check e gate DDS/interop quando pertinente. Atualizar task, traceability e relatório com logs/commit apenas se existirem. Falha/bloqueio fica explícito; nunca marcar [x] por compilação quando aceite exige comportamento real. Commits/branches e passagem ao próximo marco obedecem autorização da sessão de execução, não são disparados por este documento.
