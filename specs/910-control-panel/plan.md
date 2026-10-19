# Fase 910 — Plano técnico

## Matriz Python → Rust (feature-a-feature)

| Python | Destino Rust | Decisão |
|---|---|---|
| `supervisor.py` (Flask :8150) | `control-panel::supervisor` + `server` axum | Construir (espelho de endpoints) |
| `process_manager.py` ManagedProcess | `control-panel::process` (tokio::process, log em arquivo + tail) | Construir |
| `service_catalog.py` (módulos Python) | `control-panel::catalog` (bins Rust + llama-server) | Construir atualizado; presets Python descartados |
| `dependency_manager.py` pip | — | Descartar (bins Rust não instalam pacotes) |
| `config.py` env/paths/segredos | `control-panel::config` (env CP_*) | Construir |
| `web/` Flask+templates | `orchestrator-studio` (telas existentes) | Descartar port; API é o contrato |
| `chat_client.py` (DDSClient Python) | `control-panel::chat` (mensagens puras) + trait p/ `client` | Construir corrigido |
| `metrics.py` dashboard | endpoint `/api/dashboard` agregando supervisor+hardware | Construir fino |
| `hardware_metrics.py` | `control-panel::hardware` (/proc + nvidia-smi/rocm-smi) | Construir |
| `vm_inventory.py` | `control-panel::inventory` | Construir (paridade total) |
| `host_status.py` Ansible facts | `ansible` (run + parse) com cache TTL | Construir mínimo |
| `ansible_runner.py`/`vm_manager.py` | `control-panel::ansible` (allowlist, redact, concorrência 1) | Construir |
| `test_runner.py` | `control-panel::test_runner` | Construir |
| `cli.py` | bin `control-panel` (clap-less: args manuais como `studio-node`) | Construir |

## Arquitetura da crate

```
crates/control-panel/
  Cargo.toml            # axum/tokio/serde/thiserror/anyhow (workspace), sem feature dds no core
  src/lib.rs            # módulos + re-exports
  src/config.rs         # paths, portas, env, listas de segredos
  src/catalog.rs        # ServiceDefinition, Preset, build_command, catálogo Rust, presets
  src/process.rs        # ManagedProcess assíncrono
  src/supervisor.rs     # registro thread-safe de processos
  src/server.rs         # router axum + token opcional
  src/hardware.rs       # CPU/RAM/GPU
  src/inventory.rs      # parser Ansible INI + redação
  src/ansible.rs        # ações permitidas + runner seguro
  src/test_runner.rs    # descoberta/execução/histórico
  src/chat.rs           # construção de mensagens (puro)
  src/main.rs           # bin: supervisor HTTP + CLI status/stop-all
  tests/                # paridade: catalog, inventory, ansible, process, api, chat
```

Cada arquivo ≤250 linhas úteis (regra do workspace de programação); erros com `thiserror` na lib e `anyhow` no bin; sem `unwrap()` fora de testes.

## Decisões

1. **Crate nova, não extensão do `studio-node`**: o nó é protocolo administrativo de serviços systemd com journal próprio; o painel é supervisor local de subprocessos com catálogo/presets. Reuso é por dependência (`client` para chat futuro), não por fusão.
2. **Sem feature `dds` no núcleo**: supervisor/hardware/inventário/ansible não precisam de DDS para testar; chat DDS entra atrás de trait para o E2E DDS não bloquear os gates padrão (mesmo padrão mock/dds do `dds-contract`).
3. **Token opcional** (header `Authorization: Bearer`), espelhando `studio-node::with_token`; sem token, bind só em loopback por default.
4. **Coordenação**: antes de qualquer edição fora de `crates/control-panel` + `specs/910-control-panel`, conferir mtimes de `workflow-engine/agent/dds-dataspace`; o registro de membro no `Cargo.toml` do workspace é a única edição compartilhada desta fase.
