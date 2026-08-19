# Arquitetura da Dissertação — contexto autoritativo para a migração

Resumo do sistema conforme a **dissertação** (`tese/69a588a60776208777b2007b/dissertacao.tex`
+ figuras verificadas). Complementa o `CONTEXT.md`: é a visão do próprio autor, mais completa
que o `src/orchestrator/` explorado. **O executor deve alinhar as crates Rust a esta
arquitetura.** Figuras: ver `FIGURES.md` (atenção ao descasamento PNG↔`.tex`).

## 1. Quatro planos (Figura da arquitetura geral, arquivo `F23.png`)
- **Plano de interação:** Cliente DDS (nativo) + clientes de compatibilidade HTTP/gRPC (com
  bridges HTTP↔DDS e gRPC↔DDS).
- **Plano de dados:** o **espaço global de dados DDS** — 11 tópicos em 4 grupos (§2).
- **Plano de execução:** Agent Runtimes (múltiplas instâncias), LLM Gateway, MCP Gateway,
  provedores (llama.cpp/RTX 3080/RX 7900 XTX, OpenRouter remoto).
- **Plano de controle e suporte:** Orchestrator **Monitor** (observa, não intercepta),
  Policy Engine, QoS Monitor, Context Store, Trace Collector, Metrics Collector.
- **Princípio central:** *o orquestrador monitora e coordena por observação do espaço DDS;
  NÃO é despachante obrigatório no caminho crítico.* Agentes **reivindicam** tarefas por
  iniciativa própria (data-centric).

## 2. Os 11 tópicos (Tabela de tópicos + `F23.png`)
| Grupo | Tópico | Tipo | QoS principal |
|---|---|---|---|
| Orquestração | `Tasks` | `Task` | Reliable + TransientLocal |
| Orquestração | `AgentStates` | `AgentState` | Reliable + TransientLocal |
| Orquestração | `TaskOutputs` | `TaskOutput` | Reliable + TransientLocal |
| Inferência | `LLM.InferenceRequest` | `DDSLLMInferenceRequest` | Reliable + TransientLocal |
| Inferência | `LLM.InferenceResult` | `DDSLLMInferenceResult` | Reliable + TransientLocal |
| Inferência | `LLM.InferenceError` | `DDSLLMInferenceError` | Reliable + TransientLocal |
| Ferramentas/segurança | `ToolCall.Request` | `DDSToolCallRequest` | Reliable + TransientLocal |
| Ferramentas/segurança | `SecurityPolicy` | `DDSSecurityPolicySnapshot` | Reliable + TransientLocal |
| Observabilidade | `QoS.Metric` | `DDSQoSMetric` | Reliable + TransientLocal + KeepLast(100) |
| Observabilidade | `QoS.Violation` | `DDSQoSViolation` | Reliable + TransientLocal + KeepLast(1000) |
| Observabilidade | `QoS.Discovery` | `DDSDiscoveryEvent` | Reliable + Volatile + KeepLast(50) |

> **Nota de reconciliação:** a dissertação usa `AgentStates`/`TaskOutputs`; o código
> (`src/orchestrator`) usa `AgentRegistry`/`TaskOutput`. **A fonte de verdade da migração é o
> `OrchestratorDDS.idl`** — o executor confere os nomes reais lá (Fase 0). Além dos 11, o
> código tem `Context.Update`, `QoS.RoutingProfile` e `Trace.Event` — reconciliar na Fase 0.

## 3. Módulos e a abstração de transporte (`F31.png` — "Organização dos Módulos")
Três camadas, comunicação **só por interfaces** (todas começam com `I`):
1. **Aplicações / exposição de interfaces:** `agent_runtime`, `orchestrator`, `benchmarks`
   (+ backends externos `http_backend`, `grpc_backend`).
2. **Serviços de plataforma (execução + governança):** `llm_gateway`, `policy_engine`,
   `mcp_gateway`, `context_store`, `observability`.
3. **Infra + adaptação de transporte:** `domain` (modelo agnóstico), `dds_backend`
   (CycloneDDS), e a **Abstração de Transporte** `ITransport`/`IPublisher<T>`/
   `ISubscriber<T>`/`ITopic<T>`/`IParticipant` com **adapters DDS (default) / HTTP / gRPC**.

**Interfaces de domínio compartilhadas:** `ITask`, `ITaskLifecycle`, `IAgentState`,
`ITaskOutput`, `IInference`, `IToolCall`, `ISecurityPolicy`, `IQoSEvent`.

> **Implicação para o mapa de crates Rust:** este padrão valida a `trait DataSpaceApi` da
> `dds-dataspace` (= `ITransport` com adapters). E revela subsistemas **que faltavam no
> scaffold**: `policy-engine`, `mcp-gateway`, `context-store`, `observability` (ver §6).

## 4. Ciclo de vida da tarefa (`F25.png` — máquina de estados)
`CREATED → PENDING → CLAIMED → RUNNING → COMPLETED`. Caminhos alternativos:
`CANCELLED`, `EXPIRED` (sem heartbeat), `FAILED` (erro de inferência/ferramenta),
`TIMED_OUT` → todos para `RECOVERY_PENDING` → volta a `CLAIMED` (reatribuição).
Responsáveis: Cliente (cria/cancela), Agent Runtime (reivindica/executa), Orchestrator
Monitor (recuperação), QoS Monitor (timeouts/deadlines), LLM Gateway (falhas de inferência).
Todas as transições são **observáveis** via o tópico `Tasks` (`Reliable + TransientLocal`).

> **Nota:** o código Rust `qos-nfcm`/dissertação usam nomes de estado ligeiramente distintos
> (`ASSIGNED`≈`CLAIMED`, `DONE`≈`COMPLETED`). Padronizar na Fase 0 a partir do IDL/enum.

## 5. Implantação física (`F32.png`)
| Nó | Hardware | Módulos | Portas |
|---|---|---|---|
| MacBook cliente | macOS | `dds_client`, `observability_cli` | DDS domínio 0; multicast 7400/7410 |
| VM Orquestrador | Ubuntu 22.04 x86_64 | `orchestrator`, `policy_engine`, `context_store`, `observability`, `dds_backend` | multicast 7400/7410; TCP 8080; gRPC 50051 |
| VM CUDA | Ubuntu, RTX 3080 | `agent_runtime`, `llm_gateway`, `mcp_gateway`, `observability_agent` | gRPC 50052; HTTP health 8081 |
| **Estação ROCm** | Ubuntu, **RX 7900 XTX** | `agent_runtime`, `llm_gateway`, `mcp_gateway`, `observability_agent` | idem CUDA |
| Banco | PostgreSQL 15 | `postgres` | 5432 |
| OpenRouter | externo | LLMs de terceiros | HTTPS 443 |

DDS domínio 0; multicast `239.255.0.1:7400` (meta) / `239.255.0.2:7410` (dados); unicast p2p
efêmero. Serialização CDR. **A RX 7900 XTX (o hardware alvo da migração) roda agente + gateways.**

## 6. Estado de implementação (Tabela da dissertação, qualificação)
| Componente | Estado |
|---|---|
| Orquestrador (HUB Python): REST, TaskScheduler, AgentRegistry, DataSpace, QoS monitor | Implementado |
| `InMemoryDataSpace` (in-process) | Implementado |
| `DDSDataSpace` (DDS nativo, cyclonedds-python) | Implementado |
| `llama.cpp_dds` bridge C++ (`LLAMA_DDS=ON`) | Implementado |
| Baseline HTTP (aiohttp + SSE) | Implementado |
| Baseline gRPC (HTTP/2 streams) | **Em desenvolvimento** |
| Captura DSCP (tcpdump) / suíte de scripts | Implementado |

> **Escopo da dissertação ≠ escopo do projeto amplo.** A qualificação descreve **QoS por
> perfis (estático/DSCP)**; o **NFCM** (decisão adaptativa de QoS) é trabalho posterior
> (artigo `artigo_fuzzy_extension_qos`), já portado para Rust em `qos-nfcm`. Policy Engine,
> MCP Gateway e Context Store aparecem na arquitetura, mas nem todos foram medidos nos
> experimentos E/OP — o executor **não deve assumir** que estão 100% implementados no Python;
> confere no `src/` antes de portar (Constituição Art. III).

## 7. Ajuste ao plano de migração (crates que faltavam)
O `MIGRATION_PLAN.md` cobria agent/dataspace/orchestrator/client/llm-gateway/qos-nfcm. Esta
arquitetura acrescenta, como **fases/crates adicionais** (Fase 3+ ou paralelas):
| Nova crate | Substitui | Prioridade |
|---|---|---|
| `policy-engine` | motor de políticas (YAML→snapshot DDS `SecurityPolicy`) | 3 |
| `mcp-gateway` | gateway de ferramentas (MCP + governança) | 3–4 |
| `context-store` | persistência de contexto (DDS→PostgreSQL) | 4 |
| `observability` | QoS/Trace/Metrics collectors (DDS→PostgreSQL) | 4 |
| `compat-http` / `compat-grpc` | backends de comparação (adapters da abstração de transporte) | 4 (opcional) |
| `benchmarks` | geração de carga E1–E5/OP1–OP4 | contínuo |
Todas seguem o mesmo padrão: implementam interfaces de domínio, falam DDS via `dds-dataspace`,
e são A/B testáveis contra o Python. Detalhar em specs próprias quando chegarem na fila.

## 8. Ligações
- Catálogo visual: `FIGURES.md`. · Plano macro: `../MIGRATION_PLAN.md`. · Contexto técnico:
  `CONTEXT.md`. · Dissertação: `tese/69a588a60776208777b2007b/dissertacao.tex`.

## 9. Correção de fidelidade do snapshot Rust (T-602, 2026-08-18)

Esta seção registra o estado de implementação do runtime Rust no snapshot
`6c226b0220d43d0f090b1b051f2de9f31ea72b49`; ela não altera o escopo histórico da
qualificação descrito nas seções anteriores. A mesma correção factual foi inserida na
fonte LaTeX canônica em `69a588a60776208777b2007b/dissertacao.tex:2182-2203` e
renderizada com sucesso no PDF T-801. O checkout da tese já continha alterações de
outros trabalhos; a recuperação acrescentou somente essa subseção, sem reescrever o
conteúdo existente. Esta correção impede que a arquitetura proposta seja apresentada
como capacidade integralmente entregue pelo runtime Rust atual.

- O caminho local do agente é DDS-first: `DdsEngine` reutiliza um writer de
  `LLM.InferenceRequest` e publica `LOCAL_ONLY` por padrão. Isso prova o lado Rust do
  caminho agente→DDS; não prova que um `llama-server` externo esteve ativo neste ciclo.
- Provedores externos permanecem uma fronteira mediada pelo `llm-gateway`. O seu núcleo
  de roteamento é parcial; nenhum adaptador HTTPS de produção foi validado como aplicação
  completa neste snapshot.
- O `DataSpace` Rust materializa **16 dos 18** tópicos canônicos. `SystemMetrics` e
  `ServerStatus` ainda faltam, e a divergência do enum `ModelSpecialization` entre o IDL
  e consumidores continua aberta para a fase 700.
- MCP, snapshots de política e observabilidade têm componentes locais, mas a integração
  completa de consumo de política pelo gateway MCP ainda não está concluída.
- O deployment DDS é somente local/rede confiável neste estado. Autenticação,
  criptografia e controle de acesso DDS para exposição externa são planejados, não
  validados; portanto o runtime não é descrito como um "secure v1" completo.
