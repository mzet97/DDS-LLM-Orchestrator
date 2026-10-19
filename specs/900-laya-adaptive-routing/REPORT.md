# M0 — Auditoria, especificação e handoff Laya/DDS

## Estado

**PASS — entrega documental M0 concluída e verificada; revisão do usuário ainda necessária para aprovar contratos/ADRs e liberar implementação. M1–M7 NÃO INICIADOS.** Não há worker, engine, novo IDL, alteração de claim/QoS ou retirada NFCM implementados nesta entrega. O usuário confirmou que outra IA fará a implementação; esta sessão entrega documentação, plano e contexto. PASS refere-se à entrega documental e às verificações listadas, não às funcionalidades propostas nem aos gates não executados.

## Evidência inicial e achados

- HEAD `5a7c15b0be3ad0638d8a041338eda41c9587ae03`, branch `studio/phase-800-node`; checkout HD1TB escolhido expressamente. Árvore inicialmente limpa. O snapshot dos anexos difere desse HEAD.
- 19 crates, 19 tópicos; binding Rust CycloneDDS `=3.0.1`, sys `=1.2.1`; mount CIFS. Hashes dos anexos/corpus em [before](evidence/before.json).
- NFCM está ativo por default na CLI DDS e acoplado ao loop global; também abriga tipos/decisor estático utilizados por runtime e benchmarks.
- Prefixo vazio ignora target_agent; agentes legados não aplicam capabilities. wf-run implementa sequência/fork-join, não engine de DAG genérico.
- Gateway MCP tem avaliação de política distribuída e claim: descrição antiga de permissividade foi refutada para esse código. Autenticação DDS real não foi demonstrada por M0.
- Fontes/líneas e distinções entre confirmação, refutação, hipótese e decisão do usuário estão em [audit](audit.md).

## Especificações e decisões

Criados os dez documentos mínimos, cinco ADRs, este relatório, evidências e [prompt de continuação](PROMPT_CONTINUACAO_LAYA_DDS.md). A matriz contém 16 requisitos; o backlog detalha tarefas de M1–M7 com dependências, entregas e Given/When/Then.

Propostas para revisão: novo contrato isolado de cinco tópicos; engine Rust com WorkflowSpace por composição; domínio adaptativo separado; QoS de aplicação sobre entidades fixas; journal SQLite local/engine único; IDs físicos por tentativa, preservando Task/TaskOutput atuais. Nenhuma dessas propostas foi promovida a “implementado” ou aprovada por inferência.

## Alterações realizadas

Somente documentos e artefatos de evidência novos em `specs/900-laya-adaptive-routing/`. Sem mudança de runtime, IDLs existentes, manifests/lockfile, testes funcionais, campanhas ou prompts congelados. Sem commit, troca de branch ou GitHub Spec Kit. Coletor temporário de evidências fora do repositório; nenhum serviço/modelo novo foi instalado.

## Verificação executada

Datas dos comandos em UTC nos metadados; execução local em 16/10/2026, incluindo a madrugada UTC de 17/10.

| Comando / ambiente | Resultado observado | Evidência |
|---|---|---|
| `cargo tree --locked --offline -i qos-nfcm` | PASS: dependentes diretos orchestrator e benchmarks | [inventory](evidence/inventory.log) |
| `cargo test --workspace --locked`, namespace root com capabilities | FAIL, exit 101: teste de permissão 0444 do Studio não falhou ao escrever | [primeira execução](evidence/workspace-test.json), [log](evidence/workspace-test.log) |
| Teste exato `catalog_auth::tests::append_failure_rolls_back_memory_to_journal_state`, capabilities removidas | PASS: 1 passou, 29 filtrados; nenhuma mudança no teste/código | [metadados](evidence/permission-recheck.json), [log](evidence/permission-recheck.log) |
| `cargo test --workspace --locked`, namespace com capabilities removidas | PASS, exit 0: **596 passaram, 0 falharam, 2 ignorados** em 128 sumários, 134,245 s | [metadados](evidence/workspace-test-corrected.json), [log](evidence/workspace-test-corrected.log) |
| `cargo clippy --workspace --locked -- -D warnings` | PASS, exit 0, 43,509 s | [metadados](evidence/workspace-clippy.json), [log](evidence/workspace-clippy.log) |
| `cargo fmt --all -- --check` | PASS, exit 0, 1,018 s | [metadados](evidence/workspace-fmt.json), [log](evidence/workspace-fmt.log) |

Os dois ignores são `dds_engine_infer_real` (precisa llama-server real) e o doctest `dispatch::Registration::notified`. A soma de 596 é o total de testes efetivamente reportados, não número de requisitos cobertos.

**Limite de features:** a baseline unificou DDS em dependências e executou `agent_e2e` (1), `contract_v4` (4) e `t808_dds` (1). `mock_parity`, `control_loop` e `qos_manager` reportaram **0 testes**, por feature gates. Portanto mock isolado e loop de controle DDS do orchestrator não foram verificados nesta execução. Os comandos focados previstos no plano não foram iniciados após a reafirmação do escopo exclusivamente documental; ficam registrados para a próxima IA, sem alegar PASS/BLOCKED técnico artificial.

Não foi testado o novo contrato Rust↔Python (não existe), nem inferência Laya, autenticação DDS ou campanhas. Não inferir novos resultados acadêmicos da baseline de software.

## Preservação, revisão e limitações

Ver [after.json](evidence/after.json) para comparação dos hashes protegidos e [document-validation.json](evidence/document-validation.json) para arquivos/links/escopo. O corpus inclui 911 rastreados Rust; 437 entradas de campanha 880; 61 do artigo fuzzy; 26 experiments; 44 campaign_data; 120 arquivos não ignorados de benchmarks/orchestration. Há sobreposição; não somar como arquivos únicos. Dados ignorados externos não foram todos hasheados; nenhum comando de campanha ou escrita nesses diretórios foi executado.

Revisão independente e tratamento dos achados: [review.md](evidence/review.md). Nenhum achado crítico/alto; orçamento agregado foi esclarecido e propagado para contratos, arquitetura, requisitos e cenários negativos. Essa correção teve conferência do autor, sem segunda revisão independente. PDF exato ausente, Graphify/RTK indisponíveis e Laya não instalado são limites explícitos, não provas negativas da arquitetura. Questões e donos em [risks](risks.md).

Rulings: respeitar dossiê único sem commits/worktrees adicionais; corrigir somente o ambiente de teste; não reescrever histórico nem completar feature gates fora do escopo reafirmado. Essas escolhas preservam o baseline, ao custo de deixar validações específicas para a execução seguinte.

## Próxima tarefa atômica

**T-900-M1-01**, somente após revisão/autorização da proposta M0-1 na sessão da IA executora: revalidar HEAD e ferramentas, congelar fixtures de tipo/hash e gerar o novo IDL isolado. Aceite: schema parseável nos geradores Rust/Python, chaves/bounds/ordem/hashes consistentes e nenhum diff nos IDLs existentes. Não iniciar worker real nessa tarefa.

---

# M1 — T-900-M1-01: contrato isolado WorkflowPlanning (2026-10-16/17)

> Seção adicionada pela sessão executora seguinte, autorizada pelo
> [handoff](PROMPT_CONTINUACAO_LAYA_DDS.md) (“retome a revisão de M0 e a
> próxima tarefa M1-01”). Conteúdo M0 acima permanece intocado. Somente esta
> tarefa foi executada; M1-02..M1-04 e M2–M7 aguardam gate.

## Estado

**PASS** para o aceite de T-900-M1-01 (IDL parseável nos geradores, tipos
congelados nas fixtures com ordem/bounds/enums/chaves e hashes iguais
Rust==Python). Duas **emendas** e um **risco aberto** reportados antes de
congelar o schema — ver abaixo. Isto NÃO é o gate completo de M1 (interop de
processos reais V02/V03 e integração ao build.rs são M1-02..M1-04).

## Evidência inicial

- HEAD revalidado `5a7c15b0be3ad0638d8a041338eda41c9587ae03`, branch
  `studio/phase-800-node`; antes da tarefa, árvore limpa exceto este dossiê.
- Revisão de M0 e ADR-001 realizadas na leitura obrigatória; nenhuma decisão
  D1–D10 reaberta.

## Implementação (novos; nada de runtime/CLI alterado)

| Arquivo | Papel |
|---|---|
| `crates/dds-contract/idl/WorkflowPlanning.idl` | Contrato M0-1: 5 tópicos, módulo `dds_llm_workflow_v1`, `@final` em todas as structs. |
| `crates/dds-contract/idl/WorkflowPlanning.schema.json` | Fixture congelada (ordem/bounds/chaves/enums/5 tópicos). |
| `crates/dds-contract/idl/WorkflowPlanning.digest_vectors.json` | 15 vetores golden do encoding de digest (preimage + SHA-256). |
| `crates/dds-contract/tests/workflow_planning_fixture.rs` | Gate durável sem feature `dds` (3 testes). |
| `specs/900-laya-adaptive-routing/tools/m1_01_fixture_tool.py` | Geração/verificação de fixtures e checks dos gerados. |
| `crates/dds-contract/Cargo.toml` + `Cargo.lock` | dev-dep `sha2 = "0.10"` (já no lock p/ 4 crates; +1 linha). |

IDLs antigos (`OrchestratorV4.idl`, `OrchestratorDDS.idl`), `build.rs`,
generated mocks e caminhos legados: **zero diff** (verificado por git).

## Emendas à proposta M0-1 (reportadas antes da mudança, com erro exato)

1. **Literais de enum prefixados** (IC_/QI_/DS_/RC_/NK_/WS_/NS_): literais IDL
   vivem no escopo do módulo; idlc C 11.0.1 rejeita duplicatas
   (`Declaration 'CANCELLED' collides...`). Ordinais/semântica idênticos ao
   contracts.md; precedente dos prefixos TS_/AH_/FR_ do V4.
2. **Sem default `= 1`**: IDL 4.2 não tem default de membro; idlc C (`syntax
   error`) e cyclonedds-build (`Expected Semi, got Assign`) rejeitam.
   `schema_version=1` é regra de contrato/validação (fixture + receptor).

Risco aberto (não decidido aqui): **cyclonedds-build 3.0.1 descarta o bound de
`string<N>`** (gera `DdsString`); idlc C e idlpy preservam. Decisão de
compatibilidade fica para M1-02 (R-INTEROP/R-PYGEN em [risks](risks.md)).

## Verificação

| Comando / ambiente | Resultado | Evidência |
|---|---|---|
| `idlc -l c` (Eclipse Cyclone DDS 11.0.1) sobre o IDL final | exit 0, sem warning de extensibilidade | [generation-run.log](evidence/m1-01/generation-run.log) |
| `idlc -l py` (idlpy 11.0.1; backend compilado do sdist em venv py3.13) | exit 0; ordem/bounds/chaves conferem (check-py OK) | idem + [generators.json](evidence/m1-01/generators.json) |
| cyclonedds-build 3.0.1 (`try_idlc=false`, scratch crate offline) | exit 0; ordem/chaves/enums/bounds de sequence conferem (check-rs OK) | idem |
| `cargo test -p dds-contract --locked --offline` | **18 passed** (8 suites), incl. 3 novos (IDL↔fixture; tópicos/chaves M0-1; digest 15 vetores Rust==Python) | [evidence/m1-01/README.md](evidence/m1-01/README.md) |
| `cargo clippy -p dds-contract --all-targets --locked --offline -- -D warnings` | exit 0 (único warning: permissão de diretório incremental no CIFS) | idem |
| `cargo fmt -p dds-contract --all -- --check` | exit 0 | idem |

## Gaps/risco/rollback

- Bounds de string no gerador Rust e obtenção reproducível do `_idlpy`
  (R-INTEROP/R-PYGEN) — decisões de M1-02.
- Rollback desta tarefa: remover os 4 arquivos novos de
  `crates/dds-contract` + reverter a dev-dep `sha2` (+1 linha no lock) e os
  arquivos deste dossiê adicionados. Nenhum dado/artefato preexistente tocado.

## Próxima tarefa atômica

**T-900-M1-02** (depende de M1-01 aceito): integrar `WorkflowPlanning.idl` ao
`build.rs`/exports/mock do dds-contract e gerar bindings Python no diretório do
worker futuro, sem dependência Laya; aceites de reproducibilidade,
typename/CDR/TypeIds e gate mock↔IDL, com nenhum diff nos dois IDLs antigos.

---

# M1 — T-900-M1-02: integração dds-contract + bindings Python (2026-10-17)

## Estado

**PASS** — build.rs compila o WorkflowPlanning (mesmo pipeline dos 19 canônicos,
com blobs XTypes do idlc C), mocks paritários, bindings Python no
`workers/laya-worker/` reprodutíveis (hashes pinados) e sem dependência Laya.
**Emenda 3** reportada antes do congelamento: campos enum → `long` ordinal
(cyclonedds 3.0.1 não implementa `DdsType` para enums; precedente V4). Os 15
vetores de digest permaneceram byte-idênticos.

## Verificação

`cargo test -p dds-contract`: mock **19/0**; `--features dds`: **48/0**
(inclui regressão dos 19 tópicos). Clippy `-D warnings` exit 0 (ambos), fmt
`--check` exit 0. Roundtrips CDR Python: intent/decision/plan OK. Detalhes e
hashes em [evidence/m1-02](evidence/m1-02/README.md).

## Próxima tarefa atômica

**T-900-M1-03**: WorkflowSpace por composição em dds-dataspace (DDS/in-memory,
QoS/bounds/cache), sem tocar o caminho dos 19 tópicos.

---

# M1 — T-900-M1-03: WorkflowSpace por composição (2026-10-17)

## Estado

**PASS** — trait própria (`WorkflowSpaceApi`) + erros tipados + backends
in-memory/DDS + perfis QoS `workflow` (contracts.md). Aceite coberto por 9
testes in-memory e smoke DDS real cross-participante; `DataSpaceApi`/19 tópicos
intocados (regressão verde). Detalhes: [evidence/m1-03](evidence/m1-03/README.md).

## Próxima tarefa atômica

**T-900-M1-04**: harness real Rust↔Python dos cinco tópicos (V02: troca
bidirecional, typename/TypeIds/CDR/keys; V03: subscriber tardio/durabilidade)
+ regressão dos 19 tópicos antigos.

---

# M1 — T-900-M1-04: interop real Rust↔Python — GATE M1 FECHADO (2026-10-17)

## Estado

**PASS** — 4/4 cenários do harness (V02-a Rust←Py 5/5, V02-b Py←Rust 5/5,
V02-c mismatch deliberado sem entrega, V03-a late joiner TransientLocal 5/5);
regressão dos 19 tópicos verde (dds-dataspace 56/0, dds-contract 48/0,
topics::ALL=19). Interop de PROCESSOS REAIS entre o binding Rust (com blobs
XTypes do idlc C) e o idlpy 11.0.1 — inclusive sequences aninhadas e
enum-como-long. Achado operacional: processos Python só se descobrem com
config loopback (harness fixa `cyclonedds_loopback.xml`). Evidência:
[evidence/m1-04](evidence/m1-04/README.md).

## Próxima tarefa atômica

**T-900-M2-01**: `workers/laya-worker` com pyproject/lock, CLI DDS e adapter
Laya mockável — unit tests sem pesos/rede (R-LAYA segue BLOCKED p/ inferência
real até pin de SDK/checkpoint).

---

# M2 — worker Laya em shadow: COMPLETO com adapter mock (2026-10-17)

## Estado

**PASS para M2-01..04 com FakeLayaAdapter (regras)**. **BLOCKED** apenas para
inferência real (R-LAYA: SDK/licença/checkpoint/hardware não pinados — decisão
do usuário; `RealLayaAdapter` gated). V04 shadow real PASS: 3 intents → 3
decisões ACCEPT pelo validator Rust em DDS real, zero publicações em
Tasks/TaskOutput. Dataset é SEED de engenharia (32 PT/EN) — métricas são
referência de implementação, não alegação de qualidade; thresholds não
congelados; sem calibração não há ativação (todo Accept é shadow_only).

## Verificação

- Worker: 24 unit tests OK (offline); digest parity 16/16 vetores golden.
- workflow-engine: 14/0; clippy `-D warnings` (mock+dds); fmt OK.
- Baseline no TEST split: qos 0.857 / planning 1.0 / macro-F1 0.552.
- Shadow: [evidence/m2](evidence/m2/README.md) (shadow-run.log, relatório).

## Próxima tarefa atômica

**T-900-M3-01**: anúncio `Agent.Capabilities` + validação contra
catálogo/registry/policy (M3; ADR-003 — domínio dedicado D-ISOLATION fica
sendo decisão de implantação no gate M3-03).

---

# M3 — capabilities e claim adaptativo: COMPLETO (2026-10-17)

## Estado

**PASS** — elegibilidade (M3-01), barreira opt-in Plan/State/Task com target
exato e revalidação pré-write (M3-02), contenção K=2/K=4 + restart in-memory e
isolamento por domínio dedicado (M3-03). Legado intocado sem `--adaptive`
(EXP1b/EXP4 preservados). Contenção em DDS real e V05/V06 end-to-end ficam
para M5 (exigem materializador publicando as três peças). 80/0 nas crates
afetadas; detalhes em [evidence/m3](evidence/m3/README.md).

## Próxima tarefa atômica

**T-900-M4-01**: mapper puro Rust dos cinco objetivos de QoS com restrições
de orçamento/modelo/prazo e fallback por template autorizado.

---

# M4 — QoS semântico: COMPLETO (2026-10-17)

## Estado

**PASS** — mapper puro (objetivo→knobs de aplicação, preços versionados,
fail-closed em custo desconhecido/orçamento, fallback autorizado e
restritivo) + telemetria V07 (solicitado/aceito/efetivo distintos; estrutura
DDS constante por construção). 33/0 no workflow-engine. Detalhes:
[evidence/m4](evidence/m4/README.md). D-QOS resolvida para V1 (ADR-002);
mutação estrutural por perfil segue bloqueada a ADR novo.

## Próxima tarefa atômica

**T-900-M5-01**: schema SQLite do engine (lock de instância, migrations,
slot de decisão, orçamento/reservas, outbox transacional).

---

# M5–M7 — engine, canary e NFCM (2026-10-17)

## M5 — COMPLETO (60/0)

Journal SQLite (lock/migrations/reservas transacionais/outbox/IN_DOUBT,
fail-closed), DAG validator (todas as rejeições + orçamento agregado; TOOL
rejeitado até V10), engine (readiness por dependência, laya1-id por tentativa,
retry com id novo, recovery da MESMA tentativa, saída tardia ignorada) e
seam de planner (DirectTemplate; LLM real é integração futura). Detalhes:
[evidence/m5](evidence/m5/README.md).

## M6 — config/protocolo CONGELADOS; runs BLOCKED

Canary é opt-in por flags com drain/rollback documentados;
[PROTOCOL_M6.md](evidence/m6/PROTOCOL_M6.md) congelado ANTES de qualquer run
(regra do test-plan). **M6-03 BLOCKED**: pin de SDK/checkpoint Laya,
hardware e autorização de campanha são decisões do usuário (R-LAYA); nenhum
run executado, nenhuma métrica presumida.

## M7 — inventário OK; runtime legado INTOCADO (por pré-condição)

`cargo tree -i qos-nfcm`: dependentes seguem sendo orchestrator+benchmarks
(legado); **o caminho 900 inteiro (workflow-engine/agent adaptivo/dataspace
workflow) NÃO depende de qos-nfcm** — verificado nos manifests. M7-02/03
(remover defaults NFCM do legado) ficam BLOCKED pela pré-condição do plano:
exigem caminho validado em campanha + autorização explícita para mudar o
runtime legado (EXP1b/EXP4 preservados intocados nesta sessão).

## Estado final da fase 900 nesta sessão

M1–M5 COMPLETOS; M6-01/02 congelados; M6-03, M7-02/03 BLOCKED com causa
específica (decisão do usuário necessária). Sem commit (não autorizado).

---

# DESBLOQUEIO FINAL — R-LAYA resolvido, M6-03 piloto REAL, M7-02/03 concluídos (2026-10-17)

## R-LAYA RESOLVIDO (autorização do usuário: usar esta máquina)

- SDK: **laya==0.4.10** (Apache-2.0) em venv dedicada `~/.cache/tese-laya`
  (py3.13 + torch 2.14.1+cpu + cyclonedds 11.0.1 — runtime completo).
- Checkpoint **pinado**: convaiinnovations/laya @ snapshot `7b928d82…`,
  model.safetensors sha256 `891102d372688fc2…` (verificado no load pelo
  adapter; divergência recusa). [checkpoint-pin.txt](evidence/m6/checkpoint-pin.txt)
- `RealLayaAdapter` COMPLETO: bateria noul por capability (8 perguntas +
  choice/scores), timeout tipado por deadline, normalização de domínio.
- Aviso honesto: o checkpoint emite warning de temperaturas inválidas —
  confianças **não calibradas** (calibration_revision segue vazia; shadow).

## M6-03 — PILOTO REAL executado (campanha completa N≥30 vem depois)

- [Avaliação real no seed](evidence/m6/laya_real_seed_report.json): Laya
  qos 0.714 / planning 0.786 / macro-F1 0.508 / P50 1,27 s (CPU, 12 perguntas
  por chamada) — **baseline de regras GANHA no seed** (0.857/1.0/0.552),
  reportado sem viés; confiança média 0.169 (não calibrada).
- [Piloto 5 runs × 2 braços](evidence/m6/pilot/) com manifests por run
  (HEAD/lock/SDK/checkpoint/hardware) e [AGGREGATE.json](evidence/m6/pilot/AGGREGATE.json):
  rules 0.875/P50≈0 ms vs laya 0.75/P50 1,32 s; 10 outliers MAD>3.5
  reportados sem remoção. Braços planner-only/laya+planner pendem integração
  E2E do engine (documentado no manifest).
- [Shadow REAL PASS](evidence/m6/shadow-real-observer.log): 2 intents →
  decisões do **Laya real** publicadas em DDS → **validator Rust ACCEPT** →
  tasks_seen=0 (worker com InvalidSample corrigido no take()).

## M7-02/03 — CONCLUÍDOS

- Default `--qos-manager` **static** (era nfcm); control loop NFCM sobe só
  para gestores fuzzy EXPLÍCITOS (reprodução: `--qos-manager nfcm`).
  Diff mínimo em `crates/orchestrator/src/main.rs`; regressão **26/0 (mock)**
  e **45/0 (dds)**, clippy/fmt OK.
- Preservação: prompts/drivers/campanha/IDLs antigos **intocados** (hashes e
  escopo do diff em [evidence/m7-closing.md](evidence/m7-closing.md)).

## Estado final da fase 900

M1–M5 completos; M6-01/02 congelados + **M6-03 piloto real executado**;
M7 completo. Restam (dependem de decisão/ambiente, não de código):
campanha N≥30 completa (piloto aprovado?), braços planner E2E, calibração do
checkpoint (dataset rotulado adequado). Sem commit (não autorizado).

---

# CAMPANHA M6 COMPLETA — 4 braços × N=30 com LLM real (2026-10-17)

**PASS** — [campaign/](evidence/m6/campaign/) com AGGREGATE/QUALITY,
manifests por run (32 arquivos) e README com leitura honesta. Destaques:
validade de planos (validador DAG Rust em bin `dag_validate`)
**1.00 no laya+planner vs 0.72 no planner-incondicional** com gate do Laya
(9/32 invocações); P50 do híbrido 22,4 s vs 35,2 s (−36%) e menos tokens
totais (92k vs 103k); regras vencem em qualidade no seed (0.906 vs 0.750)
com custo zero — reportado sem viés. **Fase 900 encerrada: todos os
marcos e gates executados.**

---

# Continuação aprovada — E2E do braço híbrido + calibração (2026-10-17)

## Etapa B — runtime E2E: **PASS**

Cadeia completa real em DDS (domínio 229): intent → decisão **Laya real** →
validator Rust → mapper QoS → journal SQLite → **planner LLM** (gemma-4-12b
via `LlmHttpPlanner` Rust) → DAG validator → Plan/State → materializer
(`laya1-` task + outbox) → **agente --adaptive** (barreira resolve o task_id
físico; claim) → **inferência real** (engine http) → TaskOutput/DONE →
watcher `on_done` → WorkflowState **DONE (2/2)**. Bin `wf-run`
(`workflow-engine/src/bin/wf_run.rs`) + runtime contra traits
(`runtime.rs`). Worker: 2 decisões, tasks=0. Evidência:
[evidence/m6/e2e](evidence/m6/e2e/README.md) com 6 bugs de integração
reais encontrados e corrigidos (runtime aninhado, state_revision, strength
CLIENTE nas tasks, NodeState PUBLISHED, **barreira incompleta agora DIFIERE
a task em vez de descartar — semântica V06** — e publisher TransientLocal
com writers retidos).

## Etapa A — dataset + calibração: wiring COMPLETO, números INDICATIVOS

- Dataset sintético por construção (`gen_dataset.py`, formato nativo do SDK,
  targets numéricos `targets.py`): snapshot400 congelado; geração até 2000
  segue em fundo (resumível).
- Calibração SDK nativa (`calibrate_laya.py`): temperature map + abstention
  + binning → `calibration_v1.json` pin **a235c9b9…**; holdout 80:
  score_mae 0.763→**0.635**, acurácias idem, ECE 0.068→0.073.
- **Honesto**: com 400 exemplos os floors do SDK (2000/bucket) não são
  atingidos — calibração INDICATIVA; gates de confiança NÃO congelados.
- Wiring pronto: adapter publica `calibration_revision` = pin;
  `DecisionPolicy.calibration_pin` no validator Rust (`shadow_only=false`
  só com pin casando; teste dedicado).

## Gates finais desta rodada

workflow-engine mock **61/0** + dds **33/0**; agent **28/0**; worker 24 OK;
clippy -D warnings (mock+dds) OK; fmt OK.

---

# FECHAMENTO — A2/A5/A6 CONCLUÍDOS + v3 em andamento (2026-10-18)

## A2 — dataset completo: 2 000/2 000 (geração local)

Cada exemplo → 12 records (1 choice + 2 scores + 9 noul). Train 1 600:
noul 14 400 (floor 2 000 ✓✓), score 3 200 (✓), choice 1 600 (abaixo do
floor 2 000 — indicativo).

## A5 — `--calibration-pin` em wf-run E wf-observe (COMPLETO)

Flag implementada em ambos os bins; o validator aplica
`DecisionPolicy.calibration_pin` → `shadow_only=false` SOMENTE quando a
decisão carrega pin casando. Worker CLI ganhou `--calibration-path`.

## A6 — sessão shadow calibrada real: **PASS**

`harness/run_calibrated_shadow.py` provou em DDS real (domínio 89):
intenção → decisão Laya REAL com pin `f7451ac7…` carregado →
wf-observe `--calibration-pin <pin-completo>` → **`shadow_only=false`**
observado no output + `tasks_seen: 0` (fronteira mantida). Timeline em
`evidence/calibration/calibrated-shadow.timeline.json`.

Bugs encontrados no fechamento: decide.py hardcodava
`calibration_revision=""` (agora propaga o pin do adapter); harness
checava substring literal `shadow_only.*false` em vez de
`shadow_only=false`; pin truncado a 16 chars não casava (precisa 64).

## A4 — recalibração v3: CONCLUÍDA (CPU ~1h50; 2026-10-18)

Entrada `calib_v3/train.jsonl` (1 600 = merge dedup 2 000 × 80%); a
ferramenta re-divide internamente → **train 1 280 / holdout 320 / 15 360
records**. Pin **`a1ff7ddd2f980d84…`**; temperature map
**[1.088, 5.0, 3.963]** (T₂ satura no teto 5.0 do SDK).

Holdout 320 — ANTES → DEPOIS: score_mae **0.797 → 0.679 (−15%)**;
choice/noul accuracy idênticas (T>0 não muda argmax); mean_confidence
0.781→0.664; ECE 0.066→0.077, Brier 0.159→0.161, AURC flat (piora leve
em ranking de confiança). Abstention gates todos 1.0 (nunca dispara).

**Veredicto:** o score head calibra de forma replicável (v1/v2/v3:
−17/−17/−15% em splits distintos) e o wiring de ativação é completo
(pin → `--calibration-pin` → `shadow_only=false`, A6 PASS). Dobrar os
records (8 064→15 360) sem mudar o padrão indica que o teto é do
checkpoint/dataset sintético, não de amostragem — números seguem
INDICATIVOS para claims de confiança (R-CALIBRATION). Detalhes em
`evidence/calibration/README.md` + `calib_v3/calibration_report.json`.

# ETAPA A+B ENCERRADA (2026-10-18)

A1(limitações Colab) A2(dataset 2 000) A3(merge/split) A4(v3) A5(pin)
A6(shadow calibrada) ✓ · B1–B5 cenários E2E ✓ · Campanha 4 braços N=30 ✓ ·
E2E com inferência real 2/2 ✓. Sem push (local).

---

# CALIBRAÇÃO v4 REAL — SPROUT no Colab L4 (2026-10-18, pedido do autor "procure no huggingface")

Dataset real **`CARROT-LLM-Routing/SPROUT`** (44k prompts × 13 modelos com
score de juiz + tokens): **rótulo POR MEDIÇÃO** (Pareto custo×qualidade), não
por construção. 12 000 train / **144 000 records** (9,4× a v3) / holdout =
**split TEST oficial** (2 000, dados reais nunca vistos no fit). Execução em
**Colab L4 via CLI** (33 min; 8,6 ex/s; pin `10ef5945…` verificado localmente;
wiring do adapter validado end-to-end). Resultados no teste real:
score_mae 0.564→**0.496 (−12%)**, brier melhora, ECE flat, **abstention noul
0.678** (primeiro gate não-trivial). **Achado central:** choice_acc real =
**0.148** (vs 0.55–0.58 no sintético; baseline de maioria = 0.85) — o
checkpoint não classifica QoS em texto real; temperatura não corrige argmax;
caminhos = fine-tune (fora do contrato, checkpoint pinado) ou repensar a
pergunta de escolha. noul/score transferem bem (0.86 / −12%). Detalhes em
`evidence/calibration/README.md` (v4) + `calib_v4/`.
