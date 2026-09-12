# Studio UI — spec (frente UX/UI, revisão UI 1.0)

Escopo vigente: `Prompt_Master_SDD_Interface_UX_UI_DDS_Orchestrator_Studio_Rust.md`
(raiz do monorepo). Somente interface desktop nativa Rust (egui 0.36.2 +
eframe 0.36.2, verificados no `Cargo.lock`). Sem frontend web/Python.

## Auditoria UI-0 (verificada neste checkout)

| Base | Estado |
|---|---|
| Telas presentes (12 seções) | Visão geral, Catálogo, Nó studio-node, Inferência, Subir inferência, SSH dedicado, Agentes, Despacho, Modelos GGUF, Serviços, Catálogo compartilhado, Topologia DDS (`main.rs:24-37`) |
| Toolkit | eframe 0.36.2 + egui 0.36.2; `egui_kittest 0.36.2` adicionado como dev-dep para testes de interação |
| Componentes reutilizáveis | Parciais: `views/ssh.rs` (painel SSH), estados por módulo (`AgentsState`, `ModelsState`, `SshSession`…); sem biblioteca de componentes unificada nem tokens centrais |
| Conectores existentes | `origin.rs` (nó HTTP), `NodeRegistry`, `SharedCatalog`, `catalog_remote`, adapters de apresentação por tela |
| Testes existentes | `tests/*_wire.rs` por área + `ssh_session_live.rs`; estados com testes unitários |
| Arquivos permitidos | `crates/orchestrator-studio/**`, `crates/studio-ssh/**` (ponte já usada pela GUI), `docs/studio-ui/**`, `scripts/` de apoio |

Classificação: telas/toolkit/testes = **encontrada no checkout**;
versões = **verificada nesta execução** (`cargo search` + lock);
decisões de desenho = **proposta de interface**.

## Requisitos cobertos (UI-Rxx → fases)

UI-R01/R02 (nativo, só apresentação): todas as fases, gate por diff.
UI-R03/R04 (contexto, navegação/busca): UI-1 (shell) + UI-2.
UI-R05/R06 (tokens/temas, layout/teclado): UI-1 + UI-6.
UI-R07/R08/R09 (tabelas, agentes, destinos): UI-2 + UI-3.
UI-R10/R11 (ferramentas, modelos): UI-3. UI-R12/R13 (monitor, workflows): UI-2/UI-4.
UI-R14–R19 (estados, conflitos, operações): UI-2 + UI-4.
UI-R20/R21 (preview, capacidades): UI-5. UI-R22/R23 (contexto, segredos): transversal.
UI-R24 (semântica canônica): transversal, sem tocar runtime.
UI-R25–R28 (logs/topologia, testes, medida, docs): UI-4/UI-6 + este documento.

## Exclusões vigentes

Daemon de nó, catálogo distribuído, descoberta/varredura, SSH/transferência
de GGUF, systemd, inferência, executor de ferramentas, IDL/QoS/claim,
campanhas experimentais. Capacidade ausente = `integration-gaps.md`,
nunca backend improvisado na tela.
