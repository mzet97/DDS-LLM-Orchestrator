# Roadmap da Migração

Visão de fases, dependências e critérios de saída (exit criteria). Cada fase é uma pasta
de spec em `specs/NNN-nome/` com `spec.md` + `plan.md` + `tasks.md`.

## Grafo de dependências
```
000-dds-contract  ─┬─> 010-interop-spike ─┬─> 100-agent ──┐
   (gera tipos      │      (prova+bench)   │               ├─> 300-control-plane ─> 400-baselines
    do IDL)         │                      └─> 200-dds-dataspace ─┘
                    └────────────────────────────────────────────┘
```
- **000** desbloqueia tudo (sem tipos DDS, nada compila com `--features dds`).
- **010** valida interop + mede o baseline (decide se o ganho justifica; gate de continuação).
- **100** e **200** podem correr em paralelo após 010 (o agente pode usar um cliente/dataspace
  mínimo; a dataspace completa é pré-req do control plane).
- **300** depende de 100+200. **400** por último (baselines + desligar Python).

## Fases e critérios de saída

| Fase | Pasta | Objetivo | Exit criteria (verificável) |
|---|---|---|---|
| 0a | `000-dds-contract` | Tipos Rust do `OrchestratorDDS.idl` via idlc | `cargo build -p dds-contract --features dds` ✓; teste de round-trip de serialização por tipo; typename/chaves conferem com o IDL |
| 0b | `010-interop-spike` | Nó Rust interopera com Python+C++ nos mesmos tópicos; benchmark | Rust publica `Task`, Python consome (e vice-versa); Rust↔C++ em `LLM.*`; relatório de latência/throughput Rust-vs-Python **com números reais** |
| 1 | `100-agent` | Agente Rust assume tasks e faz ponte ao llama-server | 1 agente Rust + N Python coexistem; zero execução dupla (ownership); paridade de comportamento; ganho medido vs agente Python |
| 2 | `200-dds-dataspace` | Camada DDS completa (WaitSet, zero-copy, dashmap, writers) | API async estável; contract tests A/B (mock vs DDS vs Python); orçamento de latência de propagação de estado atingido |
| 3 | `300-control-plane` | orchestrator (axum+scheduler) + client + gateway | E2E Rust-only funciona; client sem deadlock a ≥50 concorrentes; NFCM integrado; gateway multi-worker |
| 4 | `400-baselines` | Zadeh/FCM/DHL em `qos-nfcm`; desligar Python | 5 braços comparáveis; Python equivalente arquivado; suíte E2E verde |
| 5 | `500-dds-first-hardening` | Alinhar runtime e dissertação ao caminho DDS-first | writer LLM persistente; restrição de provedor tipada; texto fiel ao código |
| 6 | `600-v1-stability` | Revisar e endurecer a API Rust/CycloneDDS e sua integração | contratos `unsafe` explícitos; lifecycle seguro; runtime DDS-first verde; matriz código↔dissertação auditada |
| 7 | `700-production-security-supply-chain` | Fechar blockers de segurança/UB, boundaries e supply chain | Dynamic XTypes/FFI sem UB por safe API; HTTP/MCP fail-closed; DDS externo autenticado ou local-only explícito; 18 tópicos; par runtime/lib reproduzível; fila Dependabot resolvida com checks frescos |

## Orçamentos de desempenho (metas — validar com bench, não afirmar sem medir)
- Propagação de estado de Task (mesmo host): **< 5 ms p99** (Python: piso ~20–70 ms).
- Claim→início de inferência: sem execução dupla sob 3 agentes disputando 100 tasks.
- Cliente: **≥ 50 clientes concorrentes** sem deadlock (Python travava em 20).
- Streaming: sustentar a taxa de chunks do llama-server sem a thread única virar gargalo.
- Uso de CPU do plano de dados sob carga: **abaixo** do Python equivalente (medir).

## Regra de gate
Ao fim de cada fase, o executor entrega um **relatório de fase** (`specs/NNN-*/REPORT.md`)
com: o que foi feito, testes, números medidos vs orçamento, desvios, e o que passa para a
próxima fase. O líder revisa antes de liberar a fase seguinte.
