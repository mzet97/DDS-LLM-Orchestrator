# Auditoria evidence-first — M0

Data: 2026-10-16. Código auditado: `5a7c15b0be3ad0638d8a041338eda41c9587ae03`, branch `studio/phase-800-node`. Todas as referências de código abaixo são relativas à raiz Rust e a esse commit; o diretório pai não é este repositório Git. O snapshot `b2399f3` dos anexos não foi tratado como HEAD. A decisão do usuário governa a evolução; o código governa o diagnóstico; documentos acadêmicos não comprovam comportamento executado.

## Proveniência e limites

- Leitura integral dos dois anexos realizada na etapa de planejamento; SHA-256 revalidado nesta execução em [before.json](evidence/before.json).
- Árvore limpa antes da criação do dossiê. O snapshot `before.json` foi coletado depois da criação inicial de `tasks.md`; por isso mostra somente este diretório novo.
- Ambiente CIFS; toolchain e dependências em [inventory.log](evidence/inventory.log). O comando de baseline do workspace pode habilitar DDS mesmo sem `--features dds`: `det-responder/Cargo.toml:11–12` e a dev-dependency em `client/Cargo.toml:44` forçam essa feature.
- `RTK.md`, relatório Graphify da raiz e relatório indicado em `src/orchestrator/graphify-out/` ausentes; executável `graphify` ausente no PATH consultado. Relações rastreadas por buscas de símbolos, manifestos e leitura dos chamadores. Nenhuma atualização de grafo necessária: não houve edição de código.
- PDF exato `dissertacao_matheus_zeitune_v2.pdf` não localizado nas buscas locais em `docs/` e `archive/`. Outros PDFs encontrados não foram tratados como essa versão. Fonte acadêmica indireta consultada: `../../Entendimento_Tecnico_Dissertacao_DDS_LLM_Orchestrator.md`, §§3, 9–12 e metodologia. Limitação de leitura primária permanece aberta.
- Não foram executadas campanhas, inferência real, implantação, autenticação de cluster ou validação de pesos Laya.

## Achados do código

| ID / classificação | Evidência no HEAD | Achado e consequência |
|---|---|---|
| A01 CONFIRMADO | `Cargo.toml:7–29`, `crates/dds-contract/src/lib.rs:24–47` | 19 membros e 19 tópicos, incluindo `Studio.NodePresence`. Não reduzir o catálogo à lista antiga de 18. |
| A02 CONFIRMADO | `Cargo.toml:41–43`, `crates/dds-contract/build.rs:29–102` | Binding fixado em crates.io; dois IDLs locais e metadados C alimentam geração em `OUT_DIR`. Um novo IDL exige suporte equivalente, não structs manuais apenas. |
| A03 CONFIRMADO | `crates/dds-contract/idl/OrchestratorV4.idl:61–89,131–141` | Task tem chave `task_id`, retry_count e semântica de prioridade 1/5/10; TaskOutput tem chave `(task_id, seq_num)` e não carrega tentativa independente. |
| A04 CONFIRMADO | `crates/agent/src/claim.rs:41–76`, `crates/agent/src/dds.rs:177,213,264–295` | Elegibilidade checada na ingestão e antes do claim; confirma assigned_agent por leitura DDS. Prefixo vazio ignora target_agent; especialização de modelo não é capability semântica. |
| A05 CONFIRMADO | `crates/orchestrator/src/main.rs:48–55,109–116,150–155` | CLI usa `nfcm` por default; valores não reconhecidos também caem em NFCM. `--fuzzy-routing` é opt-in, mas o loop decisor é iniciado independentemente dessa flag. |
| A06 CONFIRMADO | `crates/orchestrator/src/dds.rs:716–767`, `crates/dds-dataspace/src/lib.rs:690–705` | Controlador usa `QosDecider`, StabilityController e aplica knobs aos writers de Tasks. Não há isolamento por workflow nesse mecanismo. |
| A07 CONFIRMADO | `crates/orchestrator/src/qos_routing.rs:1–25,104–151`; busca `qos_routing` em [inventory.log](evidence/inventory.log) | Publica perfil global, pesos e explicação fuzzy. Encontrados streams/caches e testes; nenhum consumidor de seleção ponderada de produção encontrado na busca do workspace Rust. Isso é resultado de análise estática delimitada. |
| A08 CONFIRMADO | `crates/dds-contract/src/qos.rs:15–41,82–177`, `crates/dds-dataspace/src/qos.rs:22–65` | Perfis estruturais diferem; `tasks_with_profile` força transport priority 8/latency 50 ms. Campo semântico numa Task não muda QoS DDS. Mutabilidade declarada em código ainda depende de execução no binding/middleware concretos. |
| A09 CONFIRMADO | `crates/dds-dataspace/src/api.rs:33–244`, `crates/dds-dataspace/src/in_memory.rs:690` | DataSpaceApi atende o domínio existente. In-memory não reproduz arbitragem DDS; evitar tratar testes de mock como prova de ownership ou acrescentar outro DataSpace completo. |
| A10 CONFIRMADO | `crates/client/src/bin/wf_run.rs:1–10,30–40,145,190`, `crates/client/src/wf_assembly.rs`; `crates/orchestrator-studio/src/workflow.rs:1–5` | Workflows explícitos sequencial/fork-join; target_agent participa de EXP4. Não foi encontrado engine genérico de DAG com journal de revisão/recuperação nesses componentes. |
| A11 CONFIRMADO | `crates/agent/src/engine_dds.rs:41–78,147–206`, `crates/llm-gateway/src/lib.rs:623` | Caminho DDS de inferência correlacionado por request_id existe; gateway contém MockProvider. Não comprova um planner pronto nem todos os adaptadores externos. |
| A12 CONFIRMADO | `crates/context-store/src/local.rs:166–181,315–325`, `crates/studio-node/src/storage.rs:12–25,65–96` | Context-store é journal JSONL com flush e replay que ignora linhas corrompidas; Studio tem SQLite para operações administrativas. Nenhum deles é, hoje, journal transacional de DAG. |
| A13 REFUTADO | `crates/mcp-gateway/src/policy.rs:212–269`, `crates/mcp-gateway/src/service.rs:178–248` | A caracterização histórica de gateway apenas permissivo não descreve este código: há validação de requester, snapshot/idade/política e claim antes do dispatch. Não equivale a identidade DDS autenticada nem prova exactly-once de efeitos. |
| A14 CONFIRMADO | `crates/orchestrator/src/dds.rs:536–659`; IDL de A03 | Reaper usa publicação sem reter ownership e reconciliação de retries. retry_count continua sem ser fencing de outputs/efeitos externos. |
| A15 CONFIRMADO | `specs/880-campanha/REPORT.md:1–5` e arquivos indexados em [inventory.log](evidence/inventory.log) | Há relatório e dados de campanha posteriores aos snapshots antigos que a chamavam pendente. Existência foi verificada; resultados estatísticos e validade não foram reexecutados por M0. |
| A16 NÃO VERIFICADO | `crates/spike-interop/scripts/`, `crates/dds-contract/tests/`; [REPORT](REPORT.md) | Harnesses existentes não comprovam interop do novo contrato, que ainda não foi implementado. |
| A17 DECISÃO DO USUÁRIO | Prompt Master D1–D10 e pedido de execução M0 | Rust no núcleo; Laya Python via DDS; NFCM aposentado para evolução; baselines preservados; sem Spec Kit; apenas documentação/auditoria nesta entrega. |

## Inventário NFCM e fronteira de migração

| Categoria | Dependentes/evidência | Destino proposto; risco |
|---|---|---|
| Algoritmos históricos | `crates/qos-nfcm/src/{lib,decider,stability,fcm,zadeh,baselines}.rs` | Preservar neste marco; isolar em M7 após testes. Não treinar nem portar novos decisores fuzzy. |
| Tipos e modo estático | `orchestrator/src/main.rs:109–116`, `benchmarks/src/lib.rs:41–42` | StaticDecider, QoSMetrics, QoSDecision e QosDecider residem na crate legada. Remover a dependência cegamente quebra até o braço estático. Novo caminho terá tipos próprios do contrato sem importar qos-nfcm. |
| Runtime | `orchestrator/src/dds.rs:27,313,716–767` | Controlador e estabilidade permanecem no ambiente legado. Engine adaptativo não inicia esse loop nem depende da crate orchestrator. |
| Publicação de roteamento | `orchestrator/src/qos_routing.rs`, `dds.rs:770–790` | `QoS.RoutingProfile` fica compatível; `Routing.Decision` é novo contrato, não renomeação do global fuzzy. |
| CLI/configuração | `orchestrator/src/main.rs:48–55`; scripts acadêmicos | Inventariar usos antes de mudar defaults/flags em M7; manter comandos reproduzíveis do congelamento. |
| Regressões | `orchestrator/tests/{control_loop,qos_manager,fuzzy_routing,reaper,qos_monitor}.rs` | Preservar testes do legado; não enfraquecê-los para retirar dependências. |
| Experimentos | `benchmarks/Cargo.toml:13`, `benchmarks/src/driver.rs:721`, `specs/880-campanha/` | Manter braços e dados; nenhuma campanha rodada por M0. |
| Documentação histórica | `specs/CONTEXT.md` §§1,6,9; `specs/ROADMAP.md`; artigo no pai | Registrar superação das escolhas para a fase 900, sem reescrever a história ou os resultados. |

`cargo tree --locked --offline -i qos-nfcm` confirmou dois dependentes diretos: orchestrator e benchmarks. O escopo de busca do runtime foi o workspace atual; Python/C++ e artigo são referência histórica/interop, não alvos de remoção.

## Fontes externas e ambiente Python

Em 2026-10-16, o Python consultado tem CycloneDDS `11.0.1` e não encontra Laya. A documentação oficial demonstra `load`/`system_one`, distingue probabilidades `noul`, valor esperado `score` e confiança de resposta; calibração é necessária. O manifesto upstream consultado declara Laya `0.4.8`, Python ≥3.10 e Apache-2.0. Isso não é uma versão instalada/aprovada, nem confirma licença dos pesos.

- [API de decisões Laya](https://nandhakishorm.github.io/laya/questions-and-answers/)
- [Manifesto upstream](https://github.com/NandhaKishorM/laya/blob/main/pyproject.toml) e [licença do código](https://github.com/NandhaKishorM/laya/blob/main/LICENSE)
- [CycloneDDS Python IDL](https://cyclonedds.io/docs/cyclonedds-python/latest/idl.html): documenta geração `idlc -l py`; disponibilidade local do plugin e interop do novo schema são gates de M1.

## Baseline e preservação

Resultados executados, comandos e limitações estão em [REPORT.md](REPORT.md) e nos pares JSON/log de [evidence](evidence/README.md). Não extrapolar fmt, compilação, testes sem DDS ou leitura estática para interop/segurança real.

O manifesto de preservação agrega pares ordenados `(caminho relativo, SHA-256)` dos 911 arquivos rastreados Rust e dos conjuntos acadêmicos inventariados. As buscas externas usam `rg --files --hidden`, respeitando ignores e excluindo ambientes/cache; os hashes não cobrem todo arquivo ignorado do projeto pai. Nenhum script de campanha ou comando de escrita nesses diretórios foi executado. Comparação final em [after.json](evidence/after.json).
