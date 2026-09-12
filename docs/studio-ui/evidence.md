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
