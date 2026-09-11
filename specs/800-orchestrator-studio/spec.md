# Fase 800 — DDS Orchestrator Studio (spec)

Fonte primária: `Prompt_Master_SDD_DDS_Orchestrator_Studio_Rust_v2_Descoberta_MultiGUI.md`
(revisão 2.0, 11/09/2026). Rótulos de evidência (§2.1 do prompt):
**documentado no contexto**, **observado no checkout**, **verificado nesta execução**,
**proposto neste SDD**.

## Missão (prompt §1, §32.1)

Desktop nativo em Rust (egui) + serviço `studio-node` por máquina para cadastrar
máquinas, escolher placement de monitor/agentes/gateways/`llama-server`, gerenciar
modelos/artefatos, agentes, ferramentas, workflows e observabilidade — com DDS no
caminho operacional, SSH no administrativo, catálogo compartilhado fora da GUI e
múltiplas GUIs com revisões condicionais (§34).

## Ponto de partida observado no checkout (P0)

- Runtime Rust existente e testado: `agent` (flags `--agent-id/--slots/--model/
  --specialization/--engine mock|http|dds`, `crates/agent/src/main.rs`), `client`
  com `wf-run` (`seq_chain_v1`, `fork_join_v1`, `fork_join_serial_v1`,
  `crates/client/src/bin/wf_run.rs`), `det-responder` (backend DDS determinístico,
  `LLM.*` + `--model`, `crates/det-responder/src/main.rs`), `llm-gateway` (lib de
  roteamento `Local/Cloud` + `ProviderConstraint`, sem binário próprio),
  `mcp-gateway` (`ToolCall.Request`, mesma instância), `policy-engine`,
  `context-store` (journal JSONL), `observability`.
- Contratos: IDLs em `src/llama_cpp/dds/idl/` (+ `v4/idl/`); tipos gerados via
  `cyclonedds-idlc` em `crates/dds-contract` (Constituição Art. I — nunca à mão).
- Fases 000–400 e 600 concluídas com REPORT; 500 (`[~]` T-601) e 700 (toda `[ ]`)
  ativas — o Studio **não** as bloqueia nem as reabre; é fase nova e paralela.

## Requisitos desta fase (incremento atual)

| ID | Requisito | Aceite |
|---|---|---|
| REQ-800 | Catálogo compartilhado com revisão condicional: publicar a partir de revisão obsoleta é rejeitado antes de qualquer efeito, sem last-writer-wins silencioso (prompt §§12–13, 34; RF-31/32; G-47/48/50). | Teste: accept avança revisão; stale é rejeitado sem mutação. |
| REQ-801 | Gerações de intenção monotônicas por deployment: operação antiga nunca sobrescreve geração mais recente aceita (§13, §25). | Teste: geração menor é rejeitada; estado preservado. |
| REQ-802 | Auditoria P0 registrada: mapa reutilizar × não-duplicar do runtime existente (§2.2, §32.3). | `specs/800-*/audit.md` com file:line. |
| REQ-803 | (planejado, não deste incremento) GUI egui navegável read-only real. | G-01 parcial futuro. |
| REQ-804 | (planejado) `studio-node` com protocolo administrativo versionado. | G-05/06 futuro. |

## Fora deste incremento (honestidade, Const. Art. III)

GUI, SSH, systemd, mDNS, SQLite, nós remotos, modelos e inferência real: **propostos
neste SDD, não implementados**. Nada aqui afirma capacidade operacional além da
guarda de revisão testada.
