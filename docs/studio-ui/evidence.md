# Studio UI — evidence (comandos, resultados, limites)

## Ambiente

- eframe/egui: 0.36.2 (`Cargo.lock`, verificado). egui_kittest: 0.36.2 (dev-dep).
- Revisão base: branch `studio/phase-800-node`.

## Matriz tela × estado × modo × integração (preencher por fase)

| Tela | carregando | vazio | erro | indisponível | demo | real |
|---|---|---|---|---|---|---|
| (preencher em UI-1…UI-5) | | | | | | |

## Registro de execuções

### UI-1 — fundação visual (nesta execução, `cargo --offline`)

- `cargo test -p orchestrator-studio --lib design`: 2 verdes (tema, contraste).
- `cargo test -p orchestrator-studio --lib shell`: 4 verdes (paleta, Enter seguro, demo).
- `cargo test -p orchestrator-studio --test gallery_kittest`: 2 verdes (render dark/light, clique real).
- `cargo clippy -p orchestrator-studio --all-targets -- -D warnings`: limpo.
- `cargo fmt --check -p orchestrator-studio`: limpo.
- Binário `studio` compila com topbar + galeria + paleta + temas.
- Snapshots pixel e janela real: pendentes (kittest sem wgpu; sem captura do desktop alheio).

### UI-2 — recursos e ambientes (nesta execução, `cargo --offline`)

- `--lib`: 39 verdes (incl. `machines` 3, `agent_defs` 4, `orchestrators` 3).
- `--test machines_kittest`: 2 verdes (clique por apelido, dimensões distintas).
- Build do binário, clippy `--all-targets -D warnings` e fmt: limpos.

### UI-3 — editores e percursos (nesta execução, `cargo --offline`)

- `--lib`: 48 verdes (incl. `agent_editor` 3, `tools` 3, `pickers` 3).
- `--test editor_kittest`: 2 verdes (digitar→avançar→voltar; vazio bloqueia com erro local).
- Total do crate: 72 passed, 0 failed; build, clippy e fmt limpos.

### UI-4 — operações e colaboração (nesta execução, `cargo --offline`)

- `--lib`: 57+ verdes (incl. `workflows` 2, `operations` 5, `review` 3).
- `--test review_kittest`: 1 verde (exemplo rotulado → conflito → decisão).
- Total do crate: 83 passed, 0 failed; build, clippy e fmt limpos.

### UI-5 — preview e matriz (nesta execução, `cargo --offline`)

- `--lib`: 61 verdes (incl. `preview` 3: 16 cenários, fixtures estáveis, selo).
- Seção Demonstração no binário; matriz em `integration-gaps.md`.
- Build, clippy e fmt limpos.

### UI-6 — acessibilidade e orçamento (nesta execução, `cargo --offline`)

- `--test palette_kittest`: 2 verdes (filtrar+Enter seguro, Esc fecha).
- `--test volume_ui`: 3 verdes (log 10k 2,8 ms, filtro 10k 3,6 ms, zoom 200%).
- Contraste: 7 pares × 2 temas ≥5,86 (`a11y.md`).
- Janela real, leitor de tela, foco restaurado fim a fim: pendentes (gaps em `a11y.md`).

## Matriz tela × estado × modo × integração (UI-G50)

| Tela | carregando | vazio | erro | indisponível | demo | real |
|---|---|---|---|---|---|---|
| Ambientes | — | lista vazia + ação | — | — | selo + cenários | registro local |
| Visão geral | skeleton parcial* | — | erro visível | — | — | estados existentes |
| Máquinas | — | vazio + ação | erro de cadastro | G-INT-01 | — | registro local |
| Agentes | via Atualizar | vazio + ação | erro visível | — | — | orquestrador / rascunho local |
| Ferramentas | — | vazio + ação | — | sem executor | — | rascunho local |
| Modelos | progresso hash | vazio + ação | erro visível | G-INT-05 | — | diretório local |
| Inferência | via Enviar | — | erro visível | — | — | servidor llama |
| Orquestradores | — | vazio + ação | — | G-INT-06 | — | definição local |
| Workflows | — | vazio | — | G-INT-07 | intenções | — |
| Operações | — | vazio + filtro | — | sem canc. real | exemplo rotulado** | painel local |
| Revisão | — | vazio + exemplo | — | — | exemplo rotulado | rascunho local |
| SSH | spinner | — | falha sem vazar | — | — | bridge real |
| Galeria | — | — | — | — | tokens/estados | — |

\* `*` Esqueleto localizado onde há recarga; zero nunca confirmado.
\** Exemplo carregado só por botão explícito; tela abre vazia.
