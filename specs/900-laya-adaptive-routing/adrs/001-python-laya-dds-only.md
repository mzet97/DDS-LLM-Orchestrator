# ADR-001 — Laya Python, fronteira exclusivamente DDS

Data: 2026-10-16. Status: fronteiras DECIDIDAS PELO USUÁRIO; composição proposta para revisão M0.

## Contexto e decisão

REQ-900-002/003/004/013. Laya executa apenas classificação no worker Python e publica Routing.Decision. Intake, planner, validator, estado, materialização, políticas e agentes são Rust. O adapter usa CycloneDDS e tipos do mesmo WorkflowPlanning.idl. APIs externas HTTP existentes permanecem em seu papel; não criar HTTP/gRPC entre worker e engine.

Proposta: processo Python separado e engine Rust biblioteca+binário; extensão pequena do transporte existente por composição. Worker em shadow no M2; adapters não acessam Tasks ou ToolCall. Pin de dependências, licença de código/pesos, checkpoint hash e dataset são gates M2, ainda não aprovados/instalados.

## Alternativas rejeitadas

- Rust reimplementando Laya, Python embutido no domínio Rust ou orquestrador Python: contradizem D1/D2.
- HTTP interno por conveniência: contradiz D3.
- Classificador gerando DAG: confunde inferência tipada com planejamento generativo.

## Consequências e prova

Falha do worker não pode impedir caminho legado. M1 prova typename/key/XTypes/CDR e QoS reais; M2 prova shadow sem publicações em Tasks. Mocks não provam interop. Rollback: desligar worker/entrada adaptativa, preservando journal e registros; nenhuma mudança de baseline. Referências: [audit A02/A11/A16](../audit.md), [contratos](../contracts.md), cenários V02/V03/V04 em [test-plan](../test-plan.md).
