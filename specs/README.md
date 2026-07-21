# specs/ — Spec-Driven Development da migração

Documentos que governam a migração para Rust. **O executor começa aqui.**

## Ordem de leitura (sempre)
1. [`CONSTITUTION.md`](./CONSTITUTION.md) — regras não-negociáveis (precedência máxima).
2. [`CONTEXT.md`](./CONTEXT.md) — o sistema inteiro (Python+C++), contrato DDS, gargalos, hardware.
3. [`DISSERTACAO.md`](./DISSERTACAO.md) — **arquitetura autoritativa do autor** (4 planos, 11 tópicos,
   subsistemas, abstração de transporte, implantação, estado de implementação). Mais completo que o código.
4. [`ROADMAP.md`](./ROADMAP.md) — fases, dependências, critérios de saída, orçamentos.
5. [`../AGENTS.md`](../AGENTS.md) — o **manual de operação do executor** (o loop SDD, DoD).

Referência visual: [`FIGURES.md`](./FIGURES.md) — catálogo das figuras da dissertação (com
alerta sobre o descasamento arquivo↔legenda no `.tex`).

## Fases (cada uma: `spec.md` = o quê/por quê · `plan.md` = como · `tasks.md` = tarefas atômicas)
| # | Pasta | Entregável | Estado |
|---|---|---|---|
| 0a | [`000-dds-contract/`](./000-dds-contract/) | tipos do IDL via idlc + perfis QoS | ✅ concluída (REPORT 2026-07-14; 10+10 testes) |
| 0b | [`010-interop-spike/`](./010-interop-spike/) | interop Rust↔Python↔C++ + benchmark (GATE) | ✅ **concluída (GATE PASSOU)** — 2026-07-17; ganho 58×–156×, matriz completa |
| 1 | [`100-agent/`](./100-agent/) | agente Rust (1º alvo) | ✅ concluída (REPORT 2026-07-18; A/B 0 execução dupla; claim 4,02 tasks/s) |
| 2 | [`200-dds-dataspace/`](./200-dds-dataspace/) | camada DDS (WaitSet, zero-copy, dashmap) | ✅ concluída (REPORT 2026-07-17; propagação p99 0,077 ms; 13 testes) |
| 3 | [`300-control-plane/`](./300-control-plane/) | orchestrator + client + gateway | ✅ concluída (REPORT 2026-07-18; E2E Rust-only; 50/50 sem deadlock) |
| 4 | [`400-baselines/`](./400-baselines/) | Zadeh/FCM/DHL + consolidação | ✅ concluída (REPORT 2026-07-18; paridade exata; Python arquivado) |

> **WF-8 (subsistemas da dissertação) — ✅ concluída 2026-07-19** (plano em `../PLANO_EXECUCAO.md` §WF-8):
> crates `policy-engine` (39 testes), `context-store` (17), `mcp-gateway` (11), `observability` (15) e
> `benchmarks` (18 unit + 3 loopback DDS) verdes neste host; perfis QoS do `dds-dataspace` alinhados 1:1
> com o `dds_data_space.py`. Restam da WF-8/WF-9: `compat-http`/`compat-grpc` (opcional) e os números
> para a tese (comparativos finais no cluster).

> Atualizado em 2026-07-17 (auditoria WF-0). Plano detalhado de execução: `../PLANO_EXECUCAO.md`.

## Convenções
- **REQ-XXX**: requisito (na `spec.md`), com critério de aceite verificável.
- **T-XXX**: task (na `tasks.md`), cita o(s) REQ; `[ ]`→`[~]`→`[x]`, `[!]` bloqueada.
- Cada fase termina com `REPORT.md` (o líder revisa no gate antes da próxima).
- Já implementado (referência de paridade): `crates/qos-nfcm` (NFCM) e `crates/orch-common`.

## Como o executor avança
Achar a primeira fase não-concluída → abrir sua pasta → seguir o loop SDD do `AGENTS.md`
(test-first, DoD, honestidade). Dúvida → `[NEEDS-CLARIFICATION]` ao líder, não adivinhar.
