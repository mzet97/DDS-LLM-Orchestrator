# Evidências M4 — QoS semântico (T-900-M4-01..02)

Data: 2026-10-17 (UTC).

## Entregas

| Tarefa | Arquivo | Aceite |
|---|---|---|
| M4-01 mapper puro | `workflow-engine/src/qos_mapper.rs` | 5 objetivos → knobs de APLICAÇÃO (ADR-002); tabela de preços versionada; LOW_COST escolhe o menor custo CONHECIDO dentro do teto (desconhecido → `UnknownPrice`, estourado → `OverBudget`); orçamento zero só admite custo zero conhecido; fallback SÓ por template pré-autorizado e mais restritivo (nunca amplia); `max_tokens` nunca ampliado |
| M4-02 telemetria | `QosTelemetryRecord` + testes V07 | requested/accepted/effective_config_hash distintos e observáveis; `dds_structural_profile` CONSTANTE (endpoints fixos — writer/ownership não mudam por requisição: o mapper é função PURA sem handles DDS); fallback registra `requested=-1` (decisão nunca chegou — honesto); rejeição vira telemetria com causa |

## Verificação

- `cargo test -p workflow-engine`: **33/0** (13 mapper + 3 V07 + 8 eligibility + 12 validator + catálogo).
- V07 in-memory: dois perfis simultâneos → mapeamentos independentes, hashes distintos, estrutura DDS constante. A inspeção de ENDPOINT real (writer vivo) fica implícita pelo design: o mapper não recebe entidades DDS; a QoS estrutural é fixa nos perfis `qos::profiles::workflow` (M1-03) — nada no caminho M4 pode mutá-la.

## Decisão D-QOS (resolvida para V1)

QoS de aplicação sobre endpoints estruturais fixos (ADR-002) implementado.
Variar estrutura DDS por perfil continua BLOQUEADO a novo design/ADR (como
no M0) — nenhuma mutação de writer foi introduzida.
