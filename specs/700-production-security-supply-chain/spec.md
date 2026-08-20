# Spec 700 — Segurança de produção e supply chain Rust

## Objetivo

Eliminar os bloqueios críticos e altos encontrados na auditoria independente de
2026-08-18, tornar o runtime seguro por default, fechar as divergências entre o contrato
DDS e o código e reconciliar a fila de 16 PRs do Dependabot sem reduzir MSRV, cobertura
ou garantias de soundness.

Esta fase não considera uma build verde suficiente: cada correção precisa de teste red
→ green, execução pela superfície pública e evidência vinculada ao SHA exato.

## Precondição SDD

A fase 500 ainda contém T-601 em progresso e T-602/T-603 abertas, embora a fase 600
esteja marcada como concluída. T-801 é o único trabalho autorizado enquanto o líder
recupera a ordem do roadmap: verificar a evidência de T-601, corrigir a dissertação para
o estado real e fechar o relatório 500. T-802 e seguintes ficam bloqueadas enquanto
essa inconsistência permanecer.

## Escopo

- Biblioteca `cyclonedds-rust`, incluindo Dynamic XTypes/CDR, FFI, sanitizadores, CI,
  documentação e release seguinte a `v3.0.0-alpha.3`.
- Runtime em `src/rust`, especialmente `orchestrator`, `mcp-gateway`, `policy-engine`,
  `dds-dataspace`, `dds-contract` e o vínculo reproduzível com a biblioteca.
- Contrato IDL canônico e materialização dos 18 tópicos descritos pela dissertação.
- Fila Dependabot aberta em 2026-08-18: PRs #1, #2, #3, #5, #6, #8, #9, #10, #11,
  #12, #13, #17, #18, #19, #20 e #21.
- README, SECURITY.md, specs e dissertação afetados pelos fatos corrigidos.

## Requisitos

- **REQ-701 — DynamicData não alcança UB por API segura:** bounded strings usam o bound
  real; toda escrita nativa é falível; leitura é limitada; `value_mut` ou qualquer
  mutação segura não permite que `dynamic_publish`/serialização escreva fora do buffer.
- **REQ-702 — CDR dinâmico hostil é rejeitado:** `cdr_to_dynamic_data` normaliza e valida
  o CDR contra o descriptor antes do reader C; cleanup de stream, descriptor, buffer e
  membros nativos é RAII inclusive em erro.
- **REQ-703 — Identidade de campos dinâmicos é preservada:** publicação e serialização
  usam os nomes/ordem do schema real, nunca chaves sintéticas `field_N` quando o builder
  definiu nomes; round-trip público devolve os mesmos valores.
- **REQ-704 — HTTP seguro por default:** o orchestrator escuta em loopback por default;
  exposição externa exige opção explícita, autenticação/autorização, identidade por
  cliente e limites de corpo, mensagens, tokens, concorrência e timeout.
- **REQ-705 — Política fail-closed e tipada:** ausência de política nega/startup falha;
  `security_level` é convertido por `TryFrom` fechado em 0..=3; MCP consome
  `Security.PolicySnapshot` e vincula decisão à identidade do solicitante.
- **REQ-706 — Tool calls exatamente uma vez:** dois gateways não executam o mesmo
  `call_id`; claim/lease/read-back ou store idempotente define um vencedor antes do
  side effect. O filesystem usa operações resistentes a symlink TOCTOU.
- **REQ-707 — Par runtime/biblioteca reproduzível:** o runtime referencia uma versão ou
  SHA publicada e auditada, rastreia `Cargo.lock`, usa `--locked` em CI e não depende de
  checkout irmão mutável. `cyclonedds-src` e vendor anunciam a mesma fonte/versão.
- **REQ-708 — Contrato DDS completo:** `DataSpace` materializa os 18 tópicos canônicos,
  incluindo `SystemMetrics` e `ServerStatus`; `ModelSpecialization` é idêntico no IDL,
  no código gerado e nos consumidores Rust/C++/Python.
- **REQ-709 — Gates de segurança obrigatórios:** CI do runtime executa Rust+DDS real;
  CI da biblioteca cobre Dynamic XTypes sob ASan, caminhos pure-Rust sob Miri com strict
  provenance e Tree Borrows, inventário de `unsafe` com contratos `SAFETY`,
  Clippy/fmt/MSRV, dependências e ações fixadas.
- **REQ-710 — Dependabot tratado por risco:** nenhum PR é integrado com base/checks
  antigos; mudanças `0.x`, major, proc-macro, bindgen e GitHub Actions são validadas
  isoladamente. Upgrades incompatíveis são adiados ou substituídos explicitamente.
- **REQ-711 — Documentação factual:** README, SECURITY.md, specs e dissertação usam a
  versão, topologia, tópicos, providers, persistência, MCP/policy e estado realmente
  observados, distinguindo implementado/parcial/planejado.
- **REQ-712 — Evidência e release:** o fechamento gera matriz requisito→teste→evidência,
  revisão independente em cinco lanes e somente então uma nova prerelease da biblioteca
  e atualização reproduzível do runtime.
- **REQ-713 — Deployment DDS explícito:** o modo local/trusted-network é identificado
  como tal; exposição DDS externa exige profile com autenticação, criptografia e access
  control, identidade verificável e smoke negativo/positivo. Sem isso o serviço não é
  descrito nem iniciado como deployment seguro.

## Critérios de aceite

1. Casos adversariais de bounded string (zero, exato, excedido e sem terminador) falham
   de forma tipada ou fazem round-trip correto sob ASan; nenhum safe Rust causa OOB.
2. Corpus CDR truncado, length prefix malicioso e descriptor incompatível é rejeitado
   antes de `dds_stream_read_sample`; fuzz/corpus e ASan ficam no CI.
3. `dynamic_publish` com nomes customizados é lido por um reader real com os mesmos
   valores, e o teste falha no estado anterior.
4. Sem credencial/configuração segura, HTTP externo e MCP filesystem não executam;
   `security_level = -1` e `4` são negados.
5. Dois gateways disputando 100 chamadas produzem exatamente 100 execuções e nenhum
   side effect duplicado; tentativa de troca de symlink não escapa da raiz.
6. Um clean checkout do runtime resolve a biblioteca auditada sem diretório irmão e
   passa `cargo test --locked --workspace --all-features`.
7. Discovery/contrato comprova os 18 tópicos e igualdade de enum/TypeId entre linguagens.
8. Todos os PRs Dependabot têm decisão registrada: integrado com checks frescos,
   substituído, ou adiado com incompatibilidade objetiva. Não sobra PR vermelho sem dono.
9. Gates fmt, Clippy `-D warnings`, suites completas, Miri aplicável, ASan FFI, CodeQL,
   dependency audit e smoke público passam nos SHAs finais dos dois repositórios.
10. O relatório não declara deployment seguro sem um smoke real de identidade,
    autenticação, criptografia e autorização no limite exposto.

## Fora de escopo

- Migrar ou reescrever `llama_cpp`, alterar Ansible ou executar campanha longa de GPU.
- Adicionar providers/MCP externos não necessários para provar os limites seguros.
- Elevar MSRV de 1.85 apenas para aceitar um upgrade de benchmark.
- Mesclar automaticamente PR Dependabot, reescrever lockfile à mão ou ignorar check
  vermelho como flaky sem reproduzir após rebase.
