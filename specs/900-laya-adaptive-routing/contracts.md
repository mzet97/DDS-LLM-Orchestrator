# Contratos propostos — revisão M0-1

Status: PROPOSTA, sem IDL gerado/compilado nem compatibilidade comprovada. Fonte única futura: `crates/dds-contract/idl/WorkflowPlanning.idl`, módulo IDL `dds_llm_workflow_v1`. Preservar os dois IDLs atuais, inclusive keyedness de LLM e TaskOutput. Os cinco novos tópicos não substituem `QoS.RoutingProfile` nem criam `ToolCall.Response`.

## Convenções de wire

| Alias documental | Tipo IDL exato | Regra |
|---|---|---|
| Id | `string<64>` | Identificador ASCII não vazio; client/request/workflow/decision/agent/node. |
| TaskId | `string<70>` | `laya1-` + SHA-256 hex minúsculo (64 caracteres). |
| Digest | `string<64>` | SHA-256 hex; vazio só quando resultado ainda não existe. |
| Revision | `unsigned long` | 32 bits; começa em 1, não reciclar; overflow é erro. |
| Timestamp | `unsigned long long` | Unix epoch ns, 64 bits, não relógio monotônico. |
| Probability | `double` | Finito em [0,1]; distribuição soma 1 com tolerância 1e-6. |
| Label | `string<64>` | Identificador de catálogo/enum versionado, nunca ID efêmero como classe. |
| ModelId | `string<256>` | Nome de modelo/checkpoint, sem credenciais. |
| Ref | `string<256>` | Referência opaca validada localmente, não URL arbitrária para download. |

Todos os tipos de tópico têm `schema_version: unsigned short = 1` como valor obrigatório do campo (a sintaxe de default será validada no gerador); receptores rejeitam versões desconhecidas. Ordem dos campos nas listas abaixo é a ordem proposta de wire. Extensibilidade inicial `@final`, XCDR1; mudança incompatível exige módulo/typename/tópico V2 e migração explícita. M1 deve provar suporte das anotações/bounds pelos dois geradores antes de congelar schema.

Strings de texto são UTF-8, com limites adicionais de bytes iguais ao bound nominal; rejeitar excesso sem truncar. Limite de payload serializado por amostra: 1 MiB. Os limites abaixo são propostas de engenharia, não medidas de desempenho. Datas futuras toleram até 5 s de skew; timestamps inconsistentes ou deadlines vencidos são rejeitados. Tempo de execução local usa relógio monotônico; comparações inter-host exigem relógios sincronizados.

## Tópicos, ownership e ciclo de vida

| Tópico / typename totalmente qualificado | Chave | Publicador → leitores | Ciclo de vida |
|---|---|---|---|
| `Workflow.Intent` / `dds_llm_workflow_v1::WorkflowIntent` | request_id | cliente/ingresso Rust → intake Rust e Laya | Comando imutável; SUBMIT ou CANCEL. Retido até ack observável ou deadline; republicação deve ter mesmo conteúdo/hash. |
| `Routing.Decision` / `dds_llm_workflow_v1::RoutingDecision` | request_id, decision_id | Laya → validator Rust/observador shadow | Previsão ou erro imutável; válida só para revisões esperadas e dentro de valid_until_ns. |
| `Agent.Capabilities` / `dds_llm_workflow_v1::AgentCapabilities` | agent_id | agente Rust → intake/engine/agentes | Snapshot substituível; revision crescente, heartbeat 5 s, expires em no máximo 10 s. |
| `Workflow.Plan` / `dds_llm_workflow_v1::WorkflowPlan` | workflow_id, plan_revision | engine Rust → agentes/observadores | Plano validado imutável; retido durante workflow; journal é fonte de recuperação. |
| `Workflow.State` / `dds_llm_workflow_v1::WorkflowState` | workflow_id, plan_revision | engine Rust → agentes/cliente/observadores | Projeção monotônica por state_revision; terminal publicado antes de disposal. |

Ownership DDS Shared nos cinco tópicos. Múltiplos writers não recebem autoridade por serem writers DDS; validação aplica publisher esperado e política. Novo engine único é dono lógico de Plan/State. Permissions DDS autenticadas, quando exigidas, devem restringir cada produtor a seus tópicos; nomes de campos não autenticam participantes.

## Campos dos tipos

### WorkflowIntent

`request_id: Id @key; workflow_id: Id; client_id: Id; schema_version: unsigned short; intent_revision: Revision; command: IntentCommand; expected_plan_revision: unsigned long; prompt: string<65536>; created_at_ns: Timestamp; deadline_ns: Timestamp; security_context_ref: Ref; policy_revision: Revision; questions_revision: Revision; capability_catalog_revision: Revision; checkpoint_revision: Digest; max_tokens: unsigned long; max_cost_microunits: unsigned long long; budget_unit: Label; fallback_template_id: string<64>; payload_hash: Digest`.

`IntentCommand` = SUBMIT, CANCEL. SUBMIT inicial usa intent_revision=1 e expected_plan_revision=0; conteúdo diferente no mesmo request_id é conflito. V1 não suporta replanejamento interativo do mesmo workflow; requer novo workflow_id. CANCEL usa novo request_id, workflow_id existente e expected_plan_revision exata, prompt vazio, não passa pelo Laya e exige autorização do intake. Zero em max_cost representa orçamento zero, não ilimitado; policy define moeda/unidade e preço conhecidos ou rejeita restrição não verificável. Fallback vazio significa rejeição, não escolha automática de template.

**Orçamento agregado:** `WorkflowIntent.max_tokens` é o teto total de tokens **gerados** por todo o workflow, incluindo saída do planner e todas as tentativas de inferência; não é teto renovável por nó. `PlanNode.max_tokens` é o teto gerado por tentativa. Tokens de entrada têm bounds de tamanho próprios e entram no custo quando o pricing os cobra. A soma conservadora `reserva do planner + Σ(node.max_tokens × node.max_attempts)` deve caber no teto da intenção; caso contrário o plano é inválido, sem reduzir tentativas/saídas silenciosamente. Templates/planner podem propor plano menor em nova submissão, nunca ampliar o teto recebido.

O journal mantém saldo, reservas e consumo por workflow; antes de publicar cada chamada de planner ou tentativa, uma transação reserva seu teto junto à outbox, com chave idempotente por chamada/tentativa. Forks e retries usam o mesmo saldo. Replay não reserva duas vezes. Consumo final só libera excesso de reserva com medição confiável do adapter; timeout/crash/uso incerto contabiliza a reserva integral como consumida até reconciliação. Erro na reserva recusa publicação. Todas as operações verificam overflow.

`max_cost_microunits` é teto total na `budget_unit` aprovada, incluindo decisões Laya, planner, inferência e ferramentas quando cobrados. A policy_revision fixa tabela/revisão de preços e critérios de custo; o adapter fornece limite superior antes da execução, considerando entrada, saída e número de tentativas. Ausência de preço/limite superior confiável recusa execução com orçamento estrito, inclusive fallback; zero só permite custo conhecido igual a zero. O intake precisa admitir/reservar o custo Laya antes de liberar processamento: no fluxo futuro, o cliente/ingresso Rust autorizado publica Intent somente após admissão pela mesma autoridade de orçamento; workers aceitam apenas essas intenções admitidas. M2 shadow usa orçamento de ensaio previamente limitado, não cobra produção. Ampliar esse protocolo para ingresso DDS arbitrário exige gate próprio, não confiar em valores declarados por cliente não autorizado.

### RoutingDecision

`request_id: Id @key; decision_id: Id @key; workflow_id: Id; schema_version: unsigned short; intent_revision: Revision; producer_id: Id; model_id: ModelId; checkpoint_revision: Digest; questions_revision: Revision; policy_revision: Revision; capability_catalog_revision: Revision; qos_profile: QosIntent; qos_probabilities: sequence<ProfileProbability,5>; capability_probabilities: sequence<CapabilityProbability,16>; requires_planning_probability: Probability; priority_score: double; priority_probabilities: sequence<Probability,3>; confidence: Probability; answer_confidence: Probability; calibration_revision: string<64>; created_at_ns: Timestamp; valid_until_ns: Timestamp; decision_status: DecisionStatus; reason_code: ReasonCode; evidence_digest: Digest; payload_hash: Digest`.

`QosIntent` = BALANCED, CRITICAL, STREAM_LIKE, LOW_COST, FAILOVER. `ProfileProbability = {profile: QosIntent; probability: Probability}`; cada perfil aparece uma vez. `CapabilityProbability = {capability_id: Label; probability: Probability; answer_confidence: Probability}`; sem duplicatas. Rust deriva required_capabilities por política calibrada versionada; não confundir probabilidade com boolean.

`DecisionStatus` = OK, ERROR. `ReasonCode` = NONE, INVALID_INPUT, MODEL_UNAVAILABLE, TIMEOUT, SCHEMA_MISMATCH, POLICY_DENIED, LOW_CONFIDENCE, EXPIRED, CAPABILITY_UNAVAILABLE, OVERLOADED, CONFLICT, INVALID_PLAN, OUTPUT_INVALID, IN_DOUBT, CANCELLED. Erro usa reason != NONE; campos de previsão podem ser vazios/zero e são ignorados. Em OK, distribuições são completas/consistentes, prioridade esperada em [0,2], label escolhido via argmax com empate LOW→NORMAL→HIGH, mapeado para prioridade Task 1/5/10 apenas após política. Não arredondar score.

Confiança agregada não substitui probabilidades por capacidade. Calibration_revision vazia só é aceita como observação shadow; execução exige validação de calibração para checkpoint/perguntas/opções concretos. valid_until não excede deadline da intenção; erro/baixa confiança não permite materialização irrestrita.

### AgentCapabilities

`agent_id: Id @key; schema_version: unsigned short; announcement_revision: Revision; capability_catalog_revision: Revision; capabilities: sequence<Label,64>; model_ids: sequence<ModelId,16>; tool_versions: sequence<ToolVersion,32>; adaptive_protocol_version: unsigned short; security_context_ref: Ref; created_at_ns: Timestamp; valid_until_ns: Timestamp`.

`ToolVersion = {tool_id: Label; version: string<64>}`. Saúde, slots e especialização vêm de AgentRegistry, não são duplicados como segunda autoridade. capabilities devem pertencer ao catálogo autorizado, estar associadas ao mesmo agent_id de registry fresco e ter adaptive_protocol_version=1. Anúncio não concede permissão. Sem matching de versão/freshness, agente é inelegível.

### WorkflowPlan

`workflow_id: Id @key; plan_revision: Revision @key; schema_version: unsigned short; request_id: Id; intent_revision: Revision; effective_decision_id: Id; policy_revision: Revision; capability_catalog_revision: Revision; planner_id: ModelId; planner_revision: Digest; created_at_ns: Timestamp; deadline_ns: Timestamp; qos_profile: QosIntent; nodes: sequence<PlanNode,32>; plan_hash: Digest`.

`PlanNode = {node_id: Id; kind: NodeKind; required_capabilities: sequence<Label,16>; model_id: ModelId; target_agent_id: string<64>; prompt_template: string<8192>; dependencies: sequence<Id,32>; inputs: sequence<InputRef,16>; output_names: sequence<Label,8>; max_attempts: unsigned short; deadline_ns: Timestamp; max_tokens: unsigned long; max_output_bytes: unsigned long; security_context_ref: Ref; priority: long; side_effect_key: string<64>}`.

`NodeKind` = INFERENCE, TOOL. `InputRef = {producer_node_id: Id; output_name: Label}`. Capabilities e security ref não podem ampliar a intenção/policy. max_attempts 1–3; max_output_bytes ≤1 MiB; depth ≤16, fan-out ≤8, nós ≤32; prioridade 1/5/10; sem dependências duplicadas/cíclicas/inexistentes. Cada input referencia output declarado de predecessor. Prazo por nó não excede workflow. target_agent_id vazio permite agentes por capacidade; preenchido exige igualdade exata no modo adaptativo, sem prefix matching.

TOOL requer nome/model_id resolvido por catálogo de ferramentas e side_effect_key determinística, gateway autorizado e garantia de idempotência específica. Até esse gate, rejeitar planos TOOL. Para plano direto sem planner usar planner_id=`static-template`, planner_revision=hash do template; isso não atribui o plano ao Laya.

### WorkflowState

`workflow_id: Id @key; plan_revision: Revision @key; schema_version: unsigned short; state_revision: Revision; request_id: Id; effective_decision_id: string<64>; plan_hash: Digest; status: WorkflowStatus; reason_code: ReasonCode; nodes: sequence<NodeState,32>; created_at_ns: Timestamp; updated_at_ns: Timestamp; expires_at_ns: Timestamp; cancelled_by_request_id: string<64>; effective_qos_config_hash: Digest; fallback_template_id: string<64>`.

`WorkflowStatus` = ACCEPTED, WAITING_DECISION, WAITING_PLAN, RUNNING, DONE, FAILED, CANCELLED, IN_DOUBT. Antes de plano validado, usar plan_revision=0, nodes vazio e plan_hash vazio; agentes nunca executam a partir desse estado. Plano confirmado usa revisão 1 na V1; o estado preliminar é encerrado após a publicação do estado da revisão 1. Negação de admissão ou esgotamento de orçamento usa FAILED com POLICY_DENIED; detalhes não sensíveis de saldo/reserva ficam no journal/trace, sem mudar o wire atual de Task.

`NodeState = {node_id: Id; status: NodeStatus; attempt_id: unsigned long; task_id: string<70>; assigned_agent_id: string<64>; ready: boolean; started_at_ns: Timestamp; completed_at_ns: Timestamp; outputs: sequence<OutputRef,8>; reason_code: ReasonCode}`.

`NodeStatus` = BLOCKED, READY, PUBLISHED, ASSIGNED, RUNNING, DONE, FAILED, CANCELLED, IN_DOUBT. attempt_id=0/task_id vazio antes da primeira tentativa; depois 1..max_attempts. `OutputRef = {name: Label; context_ref: Ref; content_hash: Digest; byte_count: unsigned long; source_task_id: TaskId}`. Context refs resolvem apenas snapshots validados já existentes. `ready` é projeção da validação, nunca autorização isolada: agente confirma plano, tentativa e predecessores DONE. Estado só avança por state_revision; mesmo número com conteúdo diferente é conflito.

## QoS por entidade: proposta fixa V1

Todos: Reliability Reliable com max_blocking_time=1 s; Durability TransientLocal; History KeepLast(1) por chave; Ownership Shared; sem alteração por requisição. Readers usam políticas requested compatíveis com writers. Dispose explícito após retenção; journal/outbox preservam possibilidade de republicação após reinício. Não assumir que TransientLocal sobreviva à perda de todos os writers.

| Tópico | max_instances / max_samples / max_samples_per_instance | Deadline | Liveliness / lease | Expiração da aplicação |
|---|---|---|---|---|
| Intent | 64 / 64 / 1 | infinita | Automatic / infinita | deadline da intenção |
| Decision | 256 / 256 / 1 | infinita | Automatic / infinita | valid_until e deadline |
| Capabilities | 256 / 256 / 1 | 10 s | ManualByTopic / 10 s | valid_until ≤ created+10 s |
| Plan | 32 / 32 / 1 | infinita | Automatic / infinita | fim do workflow + retenção terminal |
| State | 32 / 32 / 1 | infinita | Automatic / infinita | fim do workflow + retenção terminal |

Retenção terminal proposta: 1 h em DDS, 7 dias no journal local; ambas configuradas antes da execução e sem apagar resultados acadêmicos. Máximo de 16 workflows ativos, 64 intenções pendentes e 4 tentativas executando por engine (além dos slots dos agentes). Cache de aplicação para novos tópicos limitado a 64 MiB, recusando admissão antes de ultrapassar; esse teto não inclui buffers nativos DDS, cujo uso precisa ser medido. Rejeição de admissão/timeout fica no estado/trace; não bloquear indefinidamente ingress/worker.

Deadline DDS dos tópicos de comando não representa prazo do workflow; prazo é validado pela aplicação. Lifespan DDS não é garantia de expiração de negócio. M1 testa limites, matching e amostras máximas; reduzir bounds exige nova revisão da proposta antes de congelamento, não truncamento em runtime.

## Mapeamento semântico de QoS

| Intenção Laya | Aplicação Rust proposta | Limitação explícita |
|---|---|---|
| BALANCED | limites padrão autorizados do ingresso | não implica nova policy DDS |
| CRITICAL | priorizar admissão/prontidão dentro de quotas, preservar deadline | sem garantia hard real-time ou preempção |
| STREAM_LIKE | pedir streaming e validar chunks incrementais | não muda History/Deadline do writer |
| LOW_COST | restringir modelo/orçamento a opções permitidas de menor custo conhecido | sem estimativa de preço inventada; custo desconhecido não satisfaz orçamento estrito |
| FAILOVER | habilitar retry dentro de max_attempts em agentes elegíveis distintos quando possível | sem exactly-once, lease distribuída ou mudança de Ownership |

Registrar perfil pedido, perfil aceito, hash de configuração DDS efetiva e motivo de fallback. Não chamar os efeitos da coluna central de mudança nativa do DDS. Se o requisito exigir estrutura DDS distinta por perfil, bloquear essa variante até ADR de endpoints compatíveis e testes de concorrência; não aproximar silenciosamente.

## Compatibilidade e gates

M1 gera Rust e Python a partir do mesmo arquivo, valida typename/chaves/bounds/XTypes/XCDR em processos reais, reinício de subscriber e incompatibilidade QoS. Mock exige cobertura própria; não inferir paridade de ownership. Nenhum schema deste documento está aprovado para publicação em runtime nesta entrega.

Mesmo ID/revisão com payload_hash diferente é erro. Preimage do hash: campos na ordem declarada, excluindo o próprio campo hash; strings UTF-8 com comprimento u32 big-endian; inteiros unsigned/longo signed com largura IDL em big-endian; enum ordinal u32 na ordem declarada; boolean u8 0/1; double IEEE-754 big-endian (normalizar -0 para +0, rejeitar NaN/infinito); sequência com comprimento u32 seguido dos elementos; structs compostas pela mesma regra. O hash do plano exclui apenas plan_hash. Essa codificação é apenas para digest, não substitui CDR no wire. M1 congela fixtures binárias e hashes idênticos Rust/Python antes de aceitar dados. Decisões concorrentes seguem o journal descrito em [architecture.md](architecture.md), sem consensus implícito.
