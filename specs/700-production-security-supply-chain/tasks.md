# Tasks 700 — Segurança de produção e supply chain Rust

- [x] **T-801 · Recuperar ordem SDD e congelar snapshots** (REQ-712)
  - Aceite: fase 500 fechada com evidência; SHAs/worktrees limpos e threat model
    registrados; nenhum patch usa o checkout sujo original da biblioteca.

- [x] **T-802 · Bounded strings e invariantes de DynamicData** (REQ-701/703)
  - Red: safe mutation + `string<4>` excedida reproduz o acesso inválido sob ASan.
  - Aceite: bound real, escrita/leitura limitada, erro tipado e custom fields corretos.

- [x] **T-803 · Normalização CDR e RAII do decode dinâmico** (REQ-702)
  - Red: CDR truncado/length prefix malicioso chega ao reader C no estado anterior.
  - Aceite: rejeição antes do FFI, cleanup em todos os erros, corpus/fuzz/ASan verdes.

- [x] **T-804 · Publicação dinâmica por schema real** (REQ-703)
  - Aceite: builder com nomes não sintéticos publica e reader DDS observa exatamente os
    valores definidos; teste anterior observa zeros e fica verde só com a correção.

- [x] **T-805 · Boundary HTTP autenticada e limitada** (REQ-704)
  - Aceite: loopback default; exposição externa sem auth não inicia; requests inválidos,
    grandes ou acima de quota falham antes do DDS; identidade não é `http-client` global.

- [x] **T-806 · Política MCP fail-closed e security level tipado** (REQ-705)
  - Aceite: sem snapshot válido nenhuma tool executa; -1/4 são negados; snapshot/delta,
    expiração e identidade do agente têm testes DDS e logs de auditoria.

- [x] **T-807 · Claim idempotente e sandbox sem TOCTOU** (REQ-706)
  - Aceite: 2 gateways/100 calls executam 100 side effects; symlink swap concorrente não
    lê/escreve fora da raiz; erros e retries não transferem ownership silenciosamente.

- [x] **T-808 · Dezoito tópicos e enum IDL único** (REQ-708)
  - Aceite: `SystemMetrics`/`ServerStatus` têm QoS, lifecycle e streams/escritas públicas;
    geração e TypeIds comprovam enum idêntico em Rust/C++/Python.

- [x] **T-809 · Integração reproduzível do candidato** (REQ-707)
  - Aceite: runtime fixa o `git rev` candidato, lock é rastreado e clone isolado
    compila/testa com `--locked` sem checkout irmão; publicação ainda não ocorre.

- [x] **T-810 · CI de segurança dos dois repositórios** (REQ-709)
  - Aceite: runtime tem CI Rust+DDS; biblioteca adiciona Dynamic XTypes ao ASan e Miri
    pure-Rust; cargo-deny/audit, CodeQL, MSRV, no_std, docs e action SHA pins verdes.
  - Fechada em 2026-08-19; runtime SHA `f467cfe07d3219b2891b6a9d369625ed186ff64a`,
    biblioteca SHA final `960b0f2e0519c81728e48321a2a402f009e5116b`; revisão
    independente em `.omo/evidence/t810-gate-review-20260819.md`.

- [x] **T-811 · Triage dos 16 PRs Dependabot** (REQ-710)
  - [x] Rebase/integrar com checks frescos: #21, #19, #17, #18.
  - [x] Rebase/testar superfícies específicas: #2, #20, #13, #1, #11.
  - [x] Rebase/pinar/testar actions: #10, #8, #6, #5.
  - [x] Fechar/substituir #3; adiar/substituir #12 sem elevar MSRV implicitamente.
  - Aceite: cada PR tem evidência e decisão; nenhum vermelho/obsoleto fica sem dono.
  - Fechada em 2026-08-19; integração no branch `candidate/t811-dependabot`,
    draft PR #24, todos os checks verdes; evidência em
    `.omo/evidence/t811-dependabot-triage-20260819.md`.

- [x] **T-812 · Reconciliar documentação e dissertação** (REQ-711)
  - Aceite: versões, 18 tópicos, DDS-first, HTTP, MCP/policy, providers, persistência,
    IDL e estado implementado/parcial/planejado batem com o snapshot testado.
  - Fechada em 2026-08-19; branch `candidate/t812-docs` nos três repositórios; revisão
    independente em `.omo/evidence/t812-documentation-reconciliation-20260819.md`.

- [x] **T-813 · Deployment DDS autenticado ou local-only explícito** (REQ-713)
  - Aceite: modo local não é anunciado como seguro externamente; modo externo tem
    autenticação, criptografia, access control e smokes de identidade permitida/negada.
  - Fechada em 2026-08-20; runtime branch `candidate/t813-security` SHA `84ef6d9`,
    draft PR #3; biblioteca branch `candidate/t813-security` SHA `7c1502f`,
    draft PR #26; smokes `intruder_participant_is_rejected` e
    `secure_participants_exchange_sample` passam localmente e no CI job
    `dds-security`; evidência em `.omo/evidence/t813-security-deployment-20260820.md`.

- [ ] **T-814 · Gate final, prerelease e relatório** (REQ-701..713)
  - Aceite: Gates A–G, QA pública, matriz requisito→teste→artefato e cinco lanes finais
    têm PASS; prerelease é publicada, runtime passa a usá-la pelo número exato e os
    gates do consumidor são repetidos antes de autorizar deploy.
