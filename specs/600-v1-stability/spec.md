# Spec 600 — Estabilidade e segurança da v1 Rust

## Objetivo

Entregar uma primeira versão estável do runtime Rust e da biblioteca `cyclonedds-rust`
usada por ele, corrigindo os defeitos de soundness, lifecycle e integração encontrados
na revisão independente. A dissertação é fonte de requisitos, não autorização para
inventar resultados nem para executar a campanha confirmatória de GPU.

## Escopo e precedência

- Esta fase começa pela conclusão técnica de T-601 da fase 500, porque o gate DDS atual
  não compila e o caminho local ainda cria um writer por inferência.
- O contrato IDL canônico e as chaves existentes não mudam.
- A biblioteca local é tratada como fonte efetivamente resolvida pelo `Cargo.lock`
  (`cyclonedds 3.0.0-alpha.1`); a divergência entre dependência local e afirmação de
  versão publicada na dissertação será registrada, não ocultada.
- Alterações na dissertação/Overleaf permanecem fora desta fase. O resultado inclui uma
  matriz de fidelidade para posterior aplicação em modo Reviewing.
- Não executar benchmarks confirmatórios ou carga prolongada de GPU.
- Preservar as alterações pré-existentes nos dois worktrees; nenhum arquivo alheio será
  revertido ou normalizado apenas por estilo.

## Requisitos

- **REQ-601 — Contratos FFI explícitos:** APIs públicas seguras não podem permitir que
  código downstream viole invariantes de layout, tipo, handle ou zero-validade usadas
  por blocos `unsafe` internos.
- **REQ-602 — Lifetime de loans:** um `WriteLoan` mantém o writer DDS vivo até o loan
  ser escrito, retornado ou descartado; código seguro não usa handle morto/reciclado.
- **REQ-603 — Entidades raw:** construtores que recebem handles sem prova tipada são
  `unsafe` e documentam integralmente suas precondições; os construtores tipados seguem
  sendo o caminho seguro.
- **REQ-604 — Filtros e callbacks:** a API segura não libera nem substitui um callback
  enquanto o CycloneDDS ainda possa invocá-lo, e nenhuma panic atravessa `extern "C"`.
- **REQ-605 — Pool de participantes:** `ParticipantPool::get` devolve a entidade
  armazenada e não confunde handle DDS com `domain_id`; esperas de discovery não mantêm
  o lock global.
- **REQ-606 — DDS-first no agente:** `DdsEngine` cria exatamente um writer de
  `LLM.InferenceRequest`, o reutiliza entre inferências e publica uma restrição tipada,
  sendo `LOCAL_ONLY` o padrão do engine DDS.
- **REQ-607 — Integração compatível:** as mudanças da biblioteca preservam a geração IDL,
  XCDR e os tipos LLM keyless; o runtime compila e passa testes DDS locais.
- **REQ-608 — Evidência honesta:** toda alegação de estabilidade é vinculada a comando,
  teste ou cenário manual executado. Limitações de Miri/ASan/ambiente são registradas.

## Critérios de aceite

1. Testes compile-fail ou equivalentes impedem implementação segura inválida de
   `DdsType` e uso seguro de construtores raw com tipos arbitrários.
2. Um teste derruba o `DataWriter` original com loan vivo e comprova que o loan mantém
   ownership suficiente para retorno/escrita segura.
3. Testes de filtros/callbacks cobrem troca/clear permitido e contenção de panic, sem
   use-after-free no modelo suportado.
4. Testes de `ParticipantPool` comprovam identidade/reuso e ausência de lock durante a
   espera de discovery.
5. Testes do agente comprovam literals `ANY`, `LOCAL_ONLY`, `CLOUD_ONLY`, default local e
   exatamente um writer antes/depois de múltiplas inferências.
6. `rustfmt --check` cobre os arquivos Rust alterados pela fase, Clippy com `-D warnings`
   e as suítes direcionadas passam nos dois workspaces. O resultado do formatter global
   também é registrado, sem normalizar a linha de base CRLF preexistente em massa.
7. Um smoke DDS real usa a API pública pelo mesmo caminho do runtime e termina com
   shutdown limpo.
8. O relatório final contém matriz requisito da dissertação → código → teste → estado
   (`implementado`, `parcial`, `planejado`), sem novos números experimentais.

## Fora de escopo

- Alterar o IDL, migrar `llama_cpp`, editar `automation/` ou executar a campanha de GPU.
- Refatorar todos os módulos acima de 250 LOC; somente splits exigidos pelas correções.
- Publicar crates, criar commits, branches, PRs ou editar o Overleaf.
