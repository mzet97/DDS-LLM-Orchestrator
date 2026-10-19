# Matriz de rastreabilidade

Status: especificação e plano; **nenhum requisito funcional novo está implementado**. A coluna Evidência atual aponta diagnóstico/baseline, nunca prova da futura função. IDs Vxx são definidos em [test-plan.md](test-plan.md). M0 e backlog de implementação em [tasks.md](tasks.md).

| REQ | Decisão / especificação | Área de implementação futura | Testes e gate | Evidência atual |
|---|---|---|---|---|
| REQ-900-001 | audit; REPORT | dossiê/evidence | V01, M0-01/02/03/06 | before/after, inventory, logs, revisão |
| REQ-900-002 | architecture; ADR-001 | workflow-engine, workers/laya-worker | V04, M2-01/02 | A11/A17; ainda proposto |
| REQ-900-003 | contracts; ADR-001 | dds-contract, dds-dataspace | V02/V03, M1-01..04 | M1-01 [x] 2026-10-16/17: IDL + fixtures congeladas + 3 geradores parseiam + gate durável (evidence/m1-01); interop de processo real (V02/V03) e integração build.rs ainda pendentes (M1-02..04) |
| REQ-900-004 | contracts; spec | validator Rust, worker Python | V04, M2-02/03 | API upstream consultada; Laya não instalado |
| REQ-900-005 | contracts; ADR-002/005 | policy mapper, engine | V07, M4-01/02 | A05/A06/A08 |
| REQ-900-006 | ADR-003 | agent, catálogo Rust | V05, M3-01/02 | A04 |
| REQ-900-007 | architecture; ADR-003 | agent, transporte, implantação adaptativa | V06, M3-02/03 | A04/A09; isolamento proposto |
| REQ-900-008 | contracts; ADR-004 | journal, validator, outbox | V09, M5-01/03 | A03/A12/A14; M1-01 [x]: encoding de digest congelado com 15 vetores golden reproduzidos Rust==Python (fixtures em crates/dds-contract/idl/) |
| REQ-900-009 | spec; contracts; ADR-004 | planner adapter, DAG validator | V08, M5-02 | A10/A11 |
| REQ-900-010 | architecture; ADR-004 | engine, journal, agentes | V08/V09, M5-03/04 | A10/A12/A14 |
| REQ-900-011 | architecture; ADR-003/004 | Policy Engine/MCP adapters | V05/V10, M3-01 e M5-04 | A13; autenticação DDS não comprovada |
| REQ-900-012 | audit; ADR-005 | paths legados preservados | V01/V12, M0-01/06 e M7-03 | manifestos/hashes e A10/A15 |
| REQ-900-013 | plan; ADR-001 | flags/ingresso/worker | V04/V11, M2-04 e M6-01 | Nenhuma ativação realizada |
| REQ-900-014 | spec; contracts | observability, queues, admission | V04/V07/V11, M2-03 e M4-02 | A08/A09; limites propostos |
| REQ-900-015 | architecture; contracts | validator, policy mapper, engine | V04/V10, M4-01 e M5-02 | fallback futuro, sem import NFCM |
| REQ-900-016 | audit; ADR-005 | runtime legado, CLI e docs | V12, M7-01..03 | cargo tree e inventário NFCM |

## Cadeia de evidência

1. `audit.md` cita arquivo/linha/HEAD; `before.json` fixa revisão e conteúdo protegido.
2. `spec.md` define aceites; ADRs justificam alternativas; `contracts.md` fixa a proposta de interface.
3. `tasks.md` liga requisitos, dependências, entregas e testes, distinguindo execução M0 do backlog M1–M7.
4. `REPORT.md` resume apenas verificações executadas; pares JSON/log preservam comando, ambiente, timestamps, exit code e SHA-256 da saída.
5. A IA executora acrescentará evidências por marco sem substituir logs M0 nem marcar V02–V12 como executados por mera existência de testes legados.
