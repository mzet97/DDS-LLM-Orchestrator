# Fase 910 — Control Panel Rust (migração de `src/control_panel`)

## Contexto e evidência

- Python atual: `src/control_panel` (~4.200 linhas, Flask). Supervisor :8150 dono de subprocessos (`supervisor.py:39,204`), catálogo que sobe módulos Python legados (`service_catalog.py:94-271`: `orchestrator.main`, `llm_gateway.main`, `agent.main`, `context_store.main`, `qos_collector.main`, `policy_engine.main`, `mcp_gateway.main` + llama-server C++), web Flask :5000 com 6 blueprints (`web/app.py:26-31`), chat DDS via `DDSClient` Python (`chat_client.py:40-48`), Ansible/VM (`vm_manager.py`, `ansible_runner.py`, `host_status.py`, `vm_inventory.py`), hardware (`hardware_metrics.py`), test runner (`test_runner.py`).
- Baseline medido 2026-10-18: `pytest src/control_panel/tests` = **42 passed** excluindo `test_chat_client.py`, que não coleta: `chat_client.py:18` importa `orchestrator.models`, inexistente no layout atual.
- Desatualização central: o catálogo sobe o runtime Python legado (desde 2026-09-10 o sancionado é `src/rust`, `notes.md:33`). Equivalente Rust: bins `orchestrator` (`crates/orchestrator/src/main.rs:223`), `agent` (`crates/agent/src/main.rs:112`), `studio-node` (`crates/studio-node/src/main.rs:59`); `llm-gateway` Rust é lib sem bin.
- Sobreposição existente (não duplicar): `studio-node` já faz HTTP axum + operações de serviços systemd + token + journal (`crates/studio-node/src/server.rs:312-343`); `studio-core` é catálogo com revisões; `studio-ssh` executa comando remoto com trust store; `orchestrator-studio` já tem 14 telas (serviços, máquinas, inferência/chat); `client` submete tasks DDS.

## Requisitos

- REQ-910-01: Crate nova `control-panel` (lib + bin `control-panel`), isolada. Nenhuma edição em `workflow-engine`, `agent`, `dds-dataspace` (em edição ativa por outra IA em 2026-10-17/18 — mtimes conferidos) sem coordenação registrada em `tasks.md`.
- REQ-910-02: Supervisor de processos equivalente ao Python: start/stop/restart/status/logs/stop-all/presets/agentes dinâmicos, thread-safe, sem processos órfãos no shutdown.
- REQ-910-03: Catálogo como dado tipado (serde), apontando para os binários Rust atuais + llama-server; presets `minimal`, `local-dds`, `full-rust` (presets Python `local-llm`/`openrouter` não portados: dependiam do gateway Python). Overrides de comando com a mesma semântica de `build_command` (`service_catalog.py:36-56`): flag existente substitui valor; booleano adiciona/remove flag.
- REQ-910-04: API HTTP (axum, padrão `studio-node`) espelhando os endpoints do supervisor Python (`supervisor.py:216-301`), com token opcional; segredos nunca serializados (`config.py:80,127`).
- REQ-910-05: Hardware CPU/RAM/GPU com as mesmas chaves de `hardware_metrics.py`; GPU ausente ⇒ lista vazia, nunca erro.
- REQ-910-06: Inventário Ansible: parser INI (grupos, hosts, `[grupo:vars]`), redação de variáveis sensíveis na serialização pública e `redact()` em texto com segredos ≥4 chars (paridade `test_vm_inventory.py`).
- REQ-910-07: Runner Ansible seguro: comando como lista (nunca shell), playbook resolvido dentro de `AUTOMATION_ROOT` (escape ⇒ erro), allowlist de ações (`config.py:95-124`), saída com segredos redigidos, limite de runs concorrentes = 1.
- REQ-910-08: Test runner: descoberta de `test_*.py`/`smoke_*.py` e de testes Rust da crate, execução com histórico JSONL — paridade comportamental com `test_runner.py:66-197`.
- REQ-910-09: Chat: montagem de mensagens (texto, visão com imagem, transcrição, tipo não suportado ⇒ erro) como função pura testável (paridade com os 5 testes de `test_chat_client.py`, hoje quebrados); transporte DDS via crate `client` atrás de trait — sem depender do `DDSClient` Python.
- REQ-910-10: Paridade e gates: todo comportamento com teste Python verde ganha teste Rust homólogo; `cargo test -p control-panel`, `clippy -- -D warnings`, `fmt --check` verdes; E2E do binário (preset sobe processo real, status, stop-all sem órfãos).
- REQ-910-11: Cutover só com E2E verde: arquivar `src/control_panel` em `archive/`, atualizar `docs/knowledge-base/02-src.md` e `notes.md`. Antes disso o Python permanece como referência.

## Não-objetivos

- Não portar os templates/JS Flask (UI é o `orchestrator-studio`).
- Não reimplementar Ansible, systemd nem DDS Security.
- Não afirmar exactly-once nem mexer no contrato de 19 tópicos.
