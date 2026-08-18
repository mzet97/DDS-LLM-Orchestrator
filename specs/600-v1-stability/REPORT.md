# Relatório 600 - candidato v1 do núcleo DDS/FFI Rust

Data: 2026-08-18

## Veredito e escopo

O núcleo local DDS/FFI está em estado de candidato v1: compila com todas as features,
passa Clippy estrito, as suites completas dos dois workspaces e os cenários públicos de
concorrência, lifecycle e shutdown executados neste ciclo. Isso não equivale a declarar
toda a arquitetura da dissertação pronta para produção. A própria dissertação registra
que a integração completa do runtime Rust permanece em andamento, e o código ainda tem
componentes parciais listados abaixo.

O termo "seguro" neste relatório significa que os contratos Rust/FFI e as fronteiras
locais auditadas falham fechadas nos casos cobertos. Não significa que o deployment DDS
tenha autenticação, criptografia e controle de acesso configurados: DDS Security ainda é
uma fronteira operacional planejada.

## Correções entregues

- `DdsType` tornou-se um contrato `unsafe`, com invariantes explícitos de layout,
  zero-validade, serialização e clone-out. O derive emite a implementação `unsafe`.
- Enums DDS gerados usam `#[repr(i32)]` e só aceitam discriminantes contíguos a partir
  de zero, preservando ABI e a validade da inicialização zerada de `Native`.
- Construtores raw de tópicos, publishers, subscribers, readers, writers e waitsets são
  escape hatches `unsafe`; APIs XTypes que criam entidades agora exigem
  `&DomainParticipant` e mantêm o participante vivo.
- `WriteLoan` mantém o writer por `Arc`; mutação de `Native` exige contrato `unsafe`;
  loans rejeitados por filtro são removidos do pool C antes do retorno.
- Loans de leitura ignoram metadados `valid_data=false` e ponteiros nulos antes de
  converter ou dereferenciar a amostra nativa.
- Callbacks de listener contêm panic na fronteira C; filtros retêm o estado do callback e
  cobrem instalação, substituição e clear no lifecycle suportado.
- `ParticipantPool` devolve o participante armazenado e não mantém o mutex durante
  esperas de discovery.
- `DdsEngine` mantém um único writer de `LLM.InferenceRequest`; provider constraint é
  tipado e o padrão DDS é `LOCAL_ONLY`.
- `DdsClientDds` usa dois pumps persistentes e duas inscrições compartilhadas, prepara
  readers antes de publicar, reporta ausência de Tokio/fechamento de pump e aborta os
  pumps no Drop.
- O gateway LLM faz parse fechado de provider constraint, respeita a classe do provider
  no failover, evita underflow concorrente no rate limiter, usa chaves de cache sem
  ambiguidade e nunca reutiliza um resultado cacheado no fluxo streaming.
- O engine HTTP do agente aceita somente endpoints loopback HTTP(S) sem credenciais.
- O gerador local `cyclonedds-build` é a fonte usada por `dds-contract`, eliminando a
  divergência que fazia a CLI falhar antes de interpretar argumentos.

## Matriz dissertação -> código -> teste -> estado

| Requisito/alegação da dissertação | Código principal | Evidência executada | Estado |
|---|---|---|---|
| Coordenação data-centric por Tasks/AgentRegistry/TaskOutput | `dds-dataspace`, `agent`, `client`, `orchestrator` | agent E2E, client 60 concorrentes, suites DDS | implementado no núcleo local |
| Espaço global com 18 tópicos canônicos | constantes/tipos em `dds-contract`; construção em `dds-dataspace` | inspeção do construtor e testes de contrato | parcial: 16/18; faltam `SystemMetrics` e `ServerStatus` no `DataSpace` Rust |
| Inferência local DDS-first sem HTTP no caminho crítico | `agent::DdsEngine`, contratos LLM e ponte C++ externa ao escopo desta fase | writer reuse e overhead DDS; teste real de llama-server ignorado explicitamente | parcial: lado Rust validado; servidor externo não foi iniciado |
| Gateway para provedores externos com roteamento/cache/failover | `llm-gateway` | 19 testes de unidade/integração, incluindo constraint, isolamento de cache, separação streaming e concorrência | parcial: núcleo de roteamento pronto; adapter HTTPS/provider de produção não está configurado como aplicação completa |
| Gateway MCP aplica política antes de ferramentas | `mcp-gateway`, `policy-engine` | sandbox filesystem, traversal e registry | parcial: ferramentas externas são `NotConfigured`; default é permissivo e o gateway não consome `Security.PolicySnapshot` |
| Política distribuída por snapshot | `policy-engine`, tipos `Security.*` | testes de snapshot/delta/republish | parcial: engine publica; integração de consumo pelo MCP não está concluída |
| Contexto distribuído e persistência | `context-store`, tópicos `Context.*` | ingestão DDS, journal, TTL e shutdown | implementado no escopo Rust local |
| Detecção por deadline/liveliness e recuperação | `dds-dataspace::monitor`, `orchestrator::{qos_monitor,reaper}` | monitor, violation, agente morto e reaper | parcial: recuperação de agente existe; violação de deadline não implementa toda a recuperação de tarefa descrita |
| QoS fuzzy e paridade com referência | `qos-nfcm`, `orchestrator::qos_routing` | suites de paridade FCM/Zadeh e control loop | implementado funcionalmente; campanha experimental confirmatória não executada |
| Observabilidade distribuída | `observability`, `Execution.Trace`, `QoS.*` | testes de sink, collector, métricas e eventos | parcial: componentes existem; cobertura fim a fim de todos os spans permanece incompleta |
| Contrato IDL/XTypes comum a Rust/C++/Python | `dds-contract`, `cyclonedds-build`, `cyclonedds` | geração, XCDR, key/typename, interop e corpus CDR | implementado para os contratos testados |
| Binding Rust seguro com loans e WaitSet compartilhado | `cyclonedds`, `dds-dataspace::SharedWaitSet` | ownership, 1.000 loans, ASan, concorrência e suites completas | implementado no núcleo auditado |
| Segurança de deployment DDS | configuração externa ao runtime | nenhuma configuração DDS Security foi validada nesta fase | planejado; rede/domínio devem ser tratados como confiáveis até implementação |
| Runtime Rust completo e operação contínua ponta a ponta | workspace inteiro e serviços externos | suites locais; sem llama-server/GPU e sem providers/MCP externos | parcial, coerente com a ressalva explícita da dissertação |

## Gates executados

Os comandos, ambientes e limitações estão registrados em
`/var/mnt/HD1TB/tese/.omo/evidence/final-core-gates-20260818.md`.

- Biblioteca: `cargo check` e `cargo clippy -D warnings` para o workspace inteiro;
  `cargo test --workspace -- --test-threads=1` verde, incluindo stress de um milhão de
  mensagens, XTypes, CDR, filters, callbacks, loans e interop entre processos.
- Runtime: check/Clippy com todos os targets/features e suite completa em série, verde;
  integração que exige llama-server externo permaneceu `ignored` por contrato.
- Miri com strict provenance e symbolic alignment passou no caminho puro selecionado.
  Proc-macro tests executam fora do interpretador Miri, limitação reportada pela própria
  ferramenta.
- Clang AddressSanitizer instrumentando Rust e C passou para ownership de writer e para
  1.000 loans rejeitados por filtro, com leak detection e halt-on-error.
- `cargo fmt --all -- --check` e `git diff --check` passaram no runtime. Os Rust files
  semânticos da biblioteca passaram `rustfmt --check`; o `git diff --check` global da
  biblioteca continua vermelho devido à linha de base CRLF preexistente e não foi
  "corrigido" por normalização em massa.

## Limitações e riscos residuais

- O build usa `cyclonedds-src` 11.0.0, enquanto o submódulo vendor registra 11.0.1.
  O aviso de build permanece até as fontes serem reconciliadas.
- A dependência local efetivamente resolvida é `cyclonedds 3.0.0-alpha.1`; o texto do
  manifest raiz que ainda descreve a crate publicada `2.0.0` está desatualizado. Este
  relatório não afirma compatibilidade com a versão publicada.
- A sandbox local não substitui autenticação/autorização DDS na rede.
- `mcp-gateway` não deve ser exposto como gateway governado até consumir snapshots de
  política e trocar os stubs por clientes MCP configurados.
- A v1 aqui não inclui campanha confirmatória de GPU, números de desempenho novos,
  edição do Overleaf, publicação de crates nem auditoria do fork C++ completo.
- As divergências de QoS de `ServerStatus` e `Context.Update` devem ser reconciliadas.
  O IDL atual também declara três valores de `ModelSpecialization`, enquanto o runtime
  usa `Transcription = 3`; esse contrato precisa ser reconciliado antes de alegar
  equivalência completa entre IDL, runtime e demais linguagens.
  A máquina conceitual da dissertação inclui `CREATED`, `CLAIMED`, `COMPLETED`,
  cancelamento, expiração e recuperação, enquanto o runtime/wire atual possui apenas
  `Pending`, `Assigned`, `Running`, `Done` e `Failed`; equivalência arquitetural completa
  não é alegada.

## Revisões independentes finais

| Lane | Estado | Evidência |
|---|---|---|
| Goal/constraint | PASS | `final7_goal`; REQ-601–608, AC-1–8 e fechamento de T-708 |
| QA manual | PASS | `final6-manual-qa.md` e transcripts `final5-*.log` |
| Code quality | PASS | `final6_code-code-review.md` |
| Security/soundness | PASS | `final6_security`; gates Miri/ASan e suites completas |
| Context/dissertação | PASS | `final6_context`; comparação PDF/IDL/código/relatório |

As cinco lanes têm veredito terminal PASS. Todos os bloqueios acionáveis encontrados
durante as rodadas anteriores foram corrigidos e reavaliados; os riscos residuais estão
classificados nas limitações deste relatório.
