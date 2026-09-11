# T-801 — Snapshots e modelo de ameaça

Data: 2026-08-18

## Identidades congeladas

| Artefato | Local isolado | SHA | Estado no momento da captura |
|---|---|---|---|
| Runtime Rust | `.worktrees/rust-phase700` | `6c226b0220d43d0f090b1b051f2de9f31ea72b49` | limpo antes das edições documentais T-801 |
| Biblioteca CycloneDDS Rust | `.worktrees/cyclonedds-phase700` | `e71c27a1ddd684de796f8a9609f41dc3f039b189` | limpo |
| Biblioteca original, não usada | `third_party/cyclonedds-rust/cyclonedds-rust` | `c16d32f244485b1a336813546bdfaa7a0ca38642` | sujo; não é fonte de patch nem de prova reproduzível |

O cenário T-601 foi executado em uma cópia temporária do runtime isolado cujas
dependências relativas resolviam para a biblioteca isolada. As duas execuções passaram;
os comandos e transcripts estão em `.omo/evidence/t801-writer-persistence-qa-20260818.md`.
O manifest do runtime ainda aponta normalmente para o checkout irmão original: isso é um
risco conhecido, não uma identidade congelada válida, e é o objetivo de REQ-707/T-809.

## Modelo de ameaça

| Fronteira / ativo | Ameaça | Estado atual | Controle exigido / dono |
|---|---|---|---|
| DDS em rede local | Participante não autenticado, leitura/injeção de tópicos | O snapshot é local/rede confiável; não é deployment externo seguro | REQ-713/T-813: identidade, autenticação, criptografia, access control e smokes permitido/negado |
| HTTP do orchestrator | Exposição externa sem identidade, quotas ou limites | Não coberto como boundary de produção nesta tarefa | REQ-704/T-805: loopback default; authN/authZ e limites explícitos antes de bind externo |
| CDR/XTypes da rede | Payload truncado, length malicioso ou schema hostil chega ao FFI | Não encerrado por T-801 | REQ-701–703/T-802–804: validação antes do FFI, RAII, corpus e ASan |
| MCP e filesystem | Execução duplicada, política permissiva, traversal/TOCTOU por symlink | Integração política/consumo ainda parcial | REQ-705–706/T-806–807: fail-closed, snapshot válido, claim idempotente e operações no-follow por diretório-capacidade |
| Identidade de cliente/agente | `agent_id`/cliente forjado ou política aplicada a identidade genérica | Não resolvido | REQ-704–706: identidade autenticada e decisão auditável vinculada ao solicitante |
| FFI/loans/callbacks | UAF, layout/ABI incorreto ou panic atravessa C | Candidato local 600 tem controles testados; não substitui gates 700 | REQ-709/T-810: Miri pure-Rust, ASan FFI, inventário `unsafe` e CI fresco |
| Supply chain | Runtime usa path dependency para checkout irmão mutável e versão/fonte CycloneDDS diverge | Detectado; overlay isolado foi exceção de QA, não solução de produto | REQ-707/T-809 e REQ-710/T-811: git rev/release imutável, lock rastreado, fonte reconciliada e decisões Dependabot |

## Recuperação e evidência

Este artefato é suficiente para retomar após cancelamento: refaz-se a verificação dos dois
SHAs e estados limpos, executa-se o cenário T-601 com timeout e URI loopback documentados,
e mantém-se os transcripts junto ao registro de evidência. Resultados de reports anteriores
não substituem uma execução vinculada aos SHAs aqui listados.
