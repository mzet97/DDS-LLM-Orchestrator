# Plano 700 — Segurança de produção e supply chain Rust

## Estratégia

Executar em ondas bloqueantes. A ordem é soundness da biblioteca → boundaries do
runtime → contrato/reprodutibilidade → supply chain → documentação/release. Cada task
começa por um teste que falha pelo mecanismo auditado; correções de CI não podem apenas
desativar o teste ou excluir o caminho problemático.

## Onda 0 — Recuperar o SDD e congelar a linha de base

1. Fechar corretamente a fase 500 e registrar por que 600 foi executada antes dela.
2. Capturar SHA/status/diff dos dois repositórios e criar worktrees limpos a partir de
   `main`; o checkout local sujo da biblioteca não é usado para implementação.
3. Registrar threat model: DDS confiável versus não confiável, HTTP exposto, filesystem,
   CDR vindo da rede, identidade do agente/cliente e fronteiras FFI.
4. Reproduzir em red os blockers antes da primeira correção.

## Onda 1 — Biblioteca: Dynamic XTypes e CDR

1. Tornar `write_value_to_native` falível e orientada pelo schema/descriptor. Extrair o
   bound real de `TYPE_BST`; validar antes da primeira escrita e usar leitura limitada.
2. Remover a possibilidade de `value_mut` quebrar invariantes que uma API segura assume:
   restringir a mutação ou revalidar obrigatoriamente na boundary de publish/serialize.
3. Fazer publish/serialize percorrer os campos reais do schema, preservando nome,
   ordinal e member id.
4. Aplicar `dds_stream_normalize` ao caminho dinâmico antes do FFI e encapsular recursos
   nativos em guards RAII para cleanup em todos os retornos.
5. Adicionar corpus adversarial, fuzz determinístico e ASan para bounded strings,
   custom fields e CDR truncado/malformado. Miri cobre somente a lógica pure-Rust.

## Onda 2 — Runtime: boundaries e exatamente-uma-vez

1. Introduzir configuração tipada de bind/auth/limites no orchestrator. Default:
   `127.0.0.1`; `0.0.0.0` exige flag explícita e provider de identidade configurado.
2. Validar `ChatRequest` antes de publicar DDS e derivar `client_id` da identidade.
3. Substituir `i32` sem validação por `SecurityLevel::try_from`; remover fallback para
   PUBLIC e `PermissivePolicy` do caminho executável por default.
4. Conectar o MCP ao snapshot distribuído, com startup deny-all até obter política
   válida, expiração/versionamento e auditoria da decisão.
5. Definir claim idempotente de `ToolCall.Request` por gateway/call_id e confirmar o
   vencedor antes do dispatch. Testar duas instâncias reais.
6. Substituir canonicalize-then-open por capability/directory-FD + no-follow, mantendo
   limites de tamanho e erros tipados.

## Onda 3 — Contrato DDS e reprodutibilidade

1. Adicionar `SystemMetrics` e `ServerStatus` ao `DataSpace`, QoS, API e lifecycle.
2. Escolher uma única definição de `ModelSpecialization`, atualizar primeiro o IDL e
   regerar consumidores; não manter variante exclusiva do Rust.
3. Preparar o commit candidato da biblioteca e usar temporariamente seu `git rev`
   imutável no teste integrado; publicação fica bloqueada até a Onda 5.
4. Trocar os path dependencies externos do runtime pelo `git rev` candidato, rastrear
   `Cargo.lock` e provar build em clone sem o monorepo pai; após publicar, substituir
   pelo número exato da prerelease e repetir o gate do consumidor.
5. Reconciliar CycloneDDS 11.0.0/11.0.1 e fazer o ABI probe referir-se à fonte linkada.

## Onda 4 — CI e fila Dependabot

Todos os 16 PRs têm base antiga; nenhum deve ser mesclado usando os checks exibidos hoje.
Primeiro atualizar/recriar a branch sobre a `main` corrigida, depois aplicar a matriz:

| PR | Mudança | Estado observado | Decisão planejada |
|---:|---|---|---|
| #21 | serde group | verde, base antiga | rebase; integrar primeiro se gates completos verdes |
| #19 | anyhow patch | verde, base antiga | rebase; baixo risco |
| #17 | syn patch | verde, base antiga | rebase; validar derive/idlc |
| #18 | quote patch | coverage antigo com double-free da baseline v1.8 | rebase; não atribuir ao quote sem reprodução |
| #20 | web-sys 0.3.97→0.3.103 | mesmo double-free antigo em coverage | tratar como breaking 0.x; validar wasm após rebase |
| #2 | clap 4.5→4.6 | verde, base antiga | rebase; validar CLIs/help/erros |
| #13 | console-subscriber 0.4→0.5 | verde, breaking tonic | branch isolada; all-features e console smoke |
| #1 | thiserror 1→2 | coverage vermelho com log expirado | major isolado; rebase, APIs de erro e doctests |
| #11 | bindgen 0.71→0.72 | Windows vermelho com log expirado | alto risco; regenerar/comparar ABI Linux+Windows |
| #12 | criterion 0.5→0.8 | conflitante; exige MSRV 1.86 | não mesclar; manter versão compatível com 1.85 ou abrir fase de MSRV |
| #10 | docker/metadata 5→6 | verde, base antiga | rebase; pin por SHA e smoke da release |
| #9 | codecov 4→6 | verde, base antiga | rebase; validar upload/permissions e pin SHA |
| #8 | docker/login 3→4 | verde, base antiga | rebase; validar OIDC/registry e pin SHA |
| #6 | CodeQL 3→4 | Windows antigo vermelho, log expirado | rebase; rerun Windows/CodeQL e pin SHA |
| #5 | docker/build-push 6→7 | verde, base antiga | rebase; build sem push e release dry-run |
| #3 | actions group | tenta Rust 1.100.0 inexistente | fechar/substituir; separar actions e manter MSRV 1.85 explícito |

### Ordem de integração

1. Patches Rust verdes: #21, #19, #17, #18, cada um isolado.
2. CLI/wasm opcionais: #2 e #20.
3. Major/0.x: #13 e #1, um por vez.
4. Bindgen #11 somente após ABI snapshots cruzados.
5. GitHub Actions #10/#9/#8/#6/#5 com SHA pinning e workflow dry-run.
6. #12 permanece adiado; #3 é substituído por PRs menores corretos.

Após cada merge, atualizar a próxima branch e exigir checks novos; não empilhar resultados
de uma base anterior. Configurar Dependabot para não agrupar `dtolnay/rust-toolchain`
com actions e impedir auto-merge de 0.x/major/proc-macro/build/FFI.

## Onda 5 — Gates, documentação e release

- **Gate A — red/green:** cada finding tem reprodução anterior e teste de regressão.
- **Gate B — biblioteca:** fmt, Clippy, suites, doctests, no_std e MSRV.
- **Gate C — soundness:** Miri strict provenance + Tree Borrows onde aplicável; ASan
  instrumentando Rust/C nos caminhos Dynamic XTypes, loans e callbacks; fuzz corpus.
- **Gate D — runtime:** workspace all-features/locked, DDS loopback, 60 clientes,
  shutdown, 18 tópicos e dois gateways sem duplicação.
- **Gate E — boundary:** HTTP externo sem credencial é negado; MCP sem snapshot não
  executa; security levels inválidos falham fechados; sandbox resiste a symlink swap.
- **Gate F — supply chain:** cargo-deny/audit, lock rastreado, actions por SHA, CodeQL,
  checks frescos e decisão para cada PR Dependabot.
- **Gate G — fidelidade:** README, SECURITY.md, specs e dissertação conferem com o
  código/testes e não declaram providers, persistência ou deployment não executados.

Depois do primeiro PASS dos Gates A–G, publicar a prerelease, trocar o runtime do `git rev`
para a versão publicada e repetir B–G no artefato do registry. O fechamento exige
`REPORT.md`, matriz de evidência e cinco lanes independentes. A release não substitui
nenhum teste anterior; ela adiciona o smoke final do consumidor externo.
