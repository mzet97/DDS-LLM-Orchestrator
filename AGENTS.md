# Manual de Operação do Executor (IA de implementação)

Você é a **IA executora** desta migração. O líder/arquiteto definiu specs detalhados em
`specs/`. Sua função é **executar** seguindo Spec-Driven Development (SDD), com disciplina
e honestidade. Este arquivo é o seu manual.

## 0. Antes de qualquer coisa (toda sessão)
1. Leia `specs/CONSTITUTION.md` (regras não-negociáveis).
2. Leia `specs/CONTEXT.md` (o sistema inteiro) — se ainda não leu nesta sessão.
3. Leia `specs/ROADMAP.md` e identifique a **fase ativa** (a primeira não concluída).
4. Abra a pasta da fase: `specs/NNN-nome/` → leia `spec.md`, `plan.md`, `tasks.md`.

## 1. O loop SDD (por task)
Para cada task em `tasks.md`, em ordem, respeitando dependências:
```
ESCOLHER a próxima task não-bloqueada  →  marcar [~] (em progresso)
 └─ RELER o(s) REQ que ela satisfaz (na spec) e o trecho relevante do plan
 └─ TEST-FIRST: escrever/ajustar o teste de aceite da task
 └─ IMPLEMENTAR o mínimo para o teste passar
 └─ VERIFICAR: cargo test + cargo clippy -- -D warnings + cargo fmt --check
 └─ se verde → marcar [x] e commitar `[T-XXX] <resumo>`; senão, continuar (não marcar)
```
Nunca marque uma task como feita sem o teste verde. Nunca pule a spec.

## 2. Definition of Done (DoD) — uma task só está "feita" se:
- [ ] O teste de aceite descrito na task existe e passa.
- [ ] `cargo test` da crate afetada: verde.
- [ ] `cargo clippy -- -D warnings`: sem warnings.
- [ ] `cargo fmt --check`: formatado.
- [ ] Se substitui comportamento Python: há teste de **paridade** (mesmo resultado).
- [ ] Doc comment (`///`) explicando o quê e citando o `REQ-XXX`.
- [ ] Sem `[NEEDS-CLARIFICATION]` pendente na task.

## 3. Quando faltar informação
- **Pare e marque `[NEEDS-CLARIFICATION: pergunta objetiva]`** na task e no relatório.
- Não invente comportamento, número, ou API. É melhor perguntar que adivinhar (Constituição
  Art. III). Continue por outra task não-bloqueada enquanto espera.

## 4. Comandos canônicos
```bash
cd tese/src/rust
cargo test -p <crate>                 # testes de uma crate
cargo test --workspace                # tudo (sem feature dds → não builda o C)
CYCLONEDDS_STATIC=1 cargo build -p <crate> --features dds # runtime DDS real (ver nota de ambiente)
cargo clippy --workspace -- -D warnings
cargo fmt --all
# gerar tipos do IDL (Fase 0):
cargo run -p cyclonedds-idlc -- --input ../../src/llama_cpp/dds/idl/OrchestratorDDS.idl \
  --output-dir crates/dds-contract/src/generated/   # caminho exato: ver specs/000-dds-contract
```
- **DDS em teste:** rode com domínio ≤ 232. Isole domínios por teste para não cruzar tráfego.
- A crate `cyclonedds` está em `third_party/cyclonedds-rust/cyclonedds-rust/` (path dep).
- **⚠️ Ambiente SMB/CIFS (o repo está num mount `smb2` que NÃO suporta symlink).** Por padrão o
  `cyclonedds-rust-sys` compila o CycloneDDS como **.so compartilhada**, e o CMake cria symlinks
  de versão (`libddsc.so.11 → .so.11.0.0`) que **falham no SMB** → o build C aborta. Use SEMPRE
  **`CYCLONEDDS_STATIC=1`** com `--features dds`: compila a `libddsc.a` estática (sem symlinks;
  funciona direto no SMB; binário autossuficiente). Isso exige o patch já aplicado no build.rs
  do `-sys` (`CYCLONEDDS_STATIC` → `BUILD_SHARED_LIBS=OFF` + `-DCMAKE_POSITION_INDEPENDENT_CODE=ON`
  + libs transitivas pthread/dl/rt/m). **Alternativa** (sem estático): manter o build fora do SMB
  com `CARGO_TARGET_DIR=$HOME/.cache/tese-rust-target` (FS local com symlink) — também remove os
  warnings de "incremental compilation … Permission denied" que o SMB gera.

## 5. Convenções de código (Rust)
- Edição 2021, `rust >= 1.85`. `#![deny(warnings)]` em crates novas quando estável.
- Erros: `thiserror` nas libs, `anyhow` nos binários. Nada de `unwrap()` fora de testes/`main`.
- Async: `tokio` (runtime multi-thread). Data-parallel puro: `rayon`.
- Concorrência: `dashmap`/atômicos/`parking_lot` — **evite lock global**.
- Hot path: **sem alocação** (loans zero-copy); mensurar antes de otimizar.
- Nomes de tópicos/perfis/métricas: **idênticos** aos do Python/IDL (ver CONTEXT.md).
- Um módulo por responsabilidade; teste no mesmo arquivo (`#[cfg(test)]`) ou `tests/`.

## 6. Interop e paridade (Constituição Art. I e II)
- Antes de substituir um componente Python, prove interop: o nó Rust e o Python coexistem
  nos mesmos tópicos. Escreva um teste de interop (pode ser cross-process com um stub).
- Paridade: replique um comportamento conhecido do Python. Ex.: `qos-nfcm` reproduz os
  números do artigo — use o mesmo padrão para cada componente.

## 7. Relatórios (rastreabilidade)
- Ao concluir uma fase, escreva `specs/NNN-nome/REPORT.md`: tasks feitas, testes, **números
  medidos vs orçamento** (ROADMAP §orçamentos), desvios, e handoff para a próxima fase.
- Mantenha o estado das tasks atualizado no `tasks.md` (checkboxes) a cada task.
- Commits pequenos, um por task, mensagem `[T-XXX] <resumo>` + trailer Co-Authored-By.

## 8. Limites (não faça)
- Não migrar/reescrever `llama_cpp` (fica C++). Não tocar `automation/`.
- Não adicionar feature nova (paridade primeiro). Não commitar em `main` nem criar branches
  sem instrução. Não expor segredos. Não afirmar resultado sem medição.
- Não resolver conflito com a Constituição no escuro — reporte ao líder.

## 9. Fluxo de uma sessão típica
1. Ler Constitution + Context + Roadmap → achar a fase ativa.
2. Abrir `specs/<fase>/tasks.md` → pegar a próxima task `[ ]` não-bloqueada.
3. Rodar o loop SDD (§1) até a task ficar `[x]`.
4. Repetir até o fim da fase → escrever `REPORT.md` → avisar o líder para o gate.
