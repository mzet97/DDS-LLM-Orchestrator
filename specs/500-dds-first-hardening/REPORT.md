# Relatório 500 — Endurecimento DDS-first

Data: 2026-08-18

## Veredito

**Fechada com escopo factual.** O snapshot Rust tem writer de inferência DDS persistente
e uma restrição de provedor tipada, com `LOCAL_ONLY` como padrão do caminho DDS. A
fidelidade documental agora declara o que o snapshot realmente cobre: 16/18 tópicos,
integrações externas/deployment de segurança incompletos e nenhuma alegação de runtime
Rust ou "secure v1" completo.

## Anomalia de ordem SDD

A linha de base capturada antes desta recuperação mostrava T-601 em progresso, T-602 e
T-603 abertas e ausência deste relatório, enquanto T-701–T-708 da fase 600 estavam todos
marcados como completos. A especificação 600 explica a causa: ela começou pela conclusão
técnica de T-601 porque o gate DDS não compilava e o writer ainda era criado por inferência.
Isso executou a correção técnica de T-601 antes do fechamento administrativo da fase 500.
Esta recuperação não reescreve essa história: ela a registra e reconcilia os estados após
verificação fresca.

## Matriz requisito → evidência → estado

| Requisito / task | Cenário e invocação | Observável binário / artefato | Estado |
|---|---|---|---|
| REQ-501; T-601 | `distrobox enter dev-fedora -- bash -lc '... cargo test -p agent --features dds --test writer_reuse writer_persiste_entre_multiplas_inferencias -- --test-threads=1'` em overlay isolado | Duas execuções independentes: `1 passed; 0 failed`; o teste observa duas requests e `before.total_count == after.total_count == 1`. Evidência: `.omo/evidence/t801-writer-persistence-qa-20260818.md`, transcripts `t801-writer-run1.typescript` e `t801-writer-run2.typescript`. | implementado e verificado |
| REQ-502; T-601 | Mesmo cenário; inspeção direta de `crates/agent/src/engine.rs`, `engine_dds.rs` e `tests/writer_reuse.rs` | As três variantes produzem `ANY`, `LOCAL_ONLY`, `CLOUD_ONLY`; default e duas requests observadas são `LOCAL_ONLY`. | implementado e verificado |
| REQ-503; T-602 | Leitura e renderização de `69a588a60776208777b2007b/dissertacao.tex:2182-2196` | Distingue agente→DDS local do gateway de provedores externos; declara que o servidor externo não foi iniciado neste ciclo. PDF T-801, páginas numeradas 112-113. | corrigido e renderizado |
| REQ-504; T-602 | Leitura e renderização de `dissertacao.tex:2198-2203` | Declara 16/18 tópicos, integração MCP/política parcial e deployment DDS local/rede confiável; não atribui segurança de produção ou resultados não materializados. PDF T-801, páginas numeradas 112-113. | corrigido e renderizado |
| T-603 | Duas passagens `pdflatex -interaction=nonstopmode -halt-on-error` em `dev-fedora`, mais leitura do PDF | Ambas saíram 0, produziram PDF de 156 páginas e log em `/var/tmp/t801-dissertacao-build/`; tasks e relatório continuam rastreáveis. | fechado |

## Validação e limitações

- O único teste comportamental específico foi executado duas vezes a partir dos SHAs
  isolados runtime `6c226b0220d43d0f090b1b051f2de9f31ea72b49` e biblioteca
  `e71c27a1ddd684de796f8a9609f41dc3f039b189`; ambos os worktrees estavam limpos antes
  da execução. O checkout original sujo da biblioteca não foi usado pelo overlay.
- A fonte LaTeX canônica já estava suja por trabalho concorrente. Esta tarefa acrescentou
  somente `\texttt{Estado do caminho DDS-first no runtime Rust}` em
  `dissertacao.tex:2182-2203`. Ela foi compilada duas vezes com `pdflatex -halt-on-error`
  no container `dev-fedora`, ambas com saída 0. O PDF (156 páginas, SHA-256
  `5a41d5d62b08bb26db9c9c146df13a051eaa899fc3a4372b2a6f837be61a115a`) e o log
  (SHA-256 `422835ddd3e10b5a90f0a15ca0149283bc3731da0ef34a76d3e7620d50035da6`)
  estão em `/var/tmp/t801-dissertacao-build/`; as páginas 112-113 foram inspecionadas
  visualmente. Warnings preexistentes de referências/layout permanecem no log, mas nenhum
  erro de fonte da subseção ocorreu.
- A conclusão não altera o relatório 600 nem converte seu candidato local em deployment
  seguro. Os riscos de 18 tópicos, enum, policy/MCP, DDS Security e dependência da
  biblioteca seguem para a fase 700.

## Handoff

Os artefatos aprovados da fase 700 foram copiados sem alteração de conteúdo e o snapshot
mais threat model da T-801 está em
`specs/700-production-security-supply-chain/T801-SNAPSHOT-THREAT-MODEL.md`. A tarefa
T-809 deve tornar a dependência runtime/biblioteca reproduzível sem checkout irmão sujo.
