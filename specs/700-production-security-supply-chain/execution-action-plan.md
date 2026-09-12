# Plano de ação SDD (execução prática)

## Objetivo
Executar a fase 700 em ordem segura, fechar blockers e gerar evidência reprodutível.

## Sequência mínima (sem pular etapas)

- **Passo 0 — Pré-fase (obrigatório):**
  - Confirmar `600-v1-stability` fechado e sem inconsistência pendente.
  - Fechar **500-dds-first-hardening** (`T-601`, `T-602`, `T-603`) antes de `T-801`.

- **Passo 1 — T-801: Reconstrução de linha de base**
  - Congelar snapshots limpos (`runtime` + `lib`).
  - Registrar SHAs/status/diff e threat model.
  - Preparar worktrees limpos a partir de `main`.

- **Passo 2 — Onda 1 (biblioteca):**
  - `T-802` → `T-803` → `T-804`
  - Saída obrigatória: red/green + ASan/LSan + Miri (onde aplicável).

- **Passo 3 — Onda 2 (runtime boundaries):**
  - `T-805` → `T-806` → `T-807`
  - Saída obrigatória: negações seguras (HTTP/MCP), claim idempotente e hardening filesystem.

- **Passo 4 — Onda 3 (contrato + reprodutibilidade):**
  - `T-808` → `T-809`
  - Saída obrigatória: 18 tópicos + enum canônico, integração `--locked` sem checkout irmão.

- **Passo 5 — Onda 4 (supply chain):**
  - `T-810` + `T-811`
  - Saída obrigatória: CI completa + decisão formal de todos os PRs Dependabot.

- **Passo 6 — Fechamento final:**
  - `T-812` → `T-813` → `T-814`
  - Saída obrigatória: `REPORT.md` final + gates A–G com evidência por lane.

## Regra de aceite por task
1. A task só vira `[x]` com evidência bruta em `.omo/evidence/...`.
2. Não permite `green` com base suja: registrar `git status` antes/depois.
3. Nenhuma correção pode “desligar” superfície de teste; precisa red e regressão.

