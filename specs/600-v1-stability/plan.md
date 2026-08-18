# Plano 600 — Estabilidade e segurança da v1 Rust

## Estratégia

Cada correção segue red → green → refactor. O menor teste que reproduz o contrato
quebrado é executado antes da implementação; depois são rodados os consumidores reais
no runtime. Mudanças de API pública são feitas na biblioteca e adaptadas no projeto na
mesma task, evitando workarounds locais.

## Ordem de execução

1. **Congelar a linha de base e separar autoria.** Registrar HEAD/status/diffs dos dois
   worktrees e limitar cada patch aos arquivos necessários. Não normalizar finais de
   linha nem tocar nos arquivos modificados pelo usuário fora do caminho exercitado.
2. **Desbloquear o build DDS.** Corrigir a adaptação de `SharedWaitSet::new` à API tipada
   de participant, com teste de compilação/integração que falha no estado inicial.
3. **Endurecer tipos e handles da biblioteca.** Tornar explícitos os contratos de
   `DdsType` e construtores raw; atualizar derive/codegen e consumidores, acompanhados
   por testes negativos e positivos.
4. **Endurecer lifecycle.** Fazer `WriteLoan` reter ownership do writer, corrigir
   `ParticipantPool` e restringir o ciclo de vida de filtros/callbacks; validar Drop,
   panic barrier e concorrência suportada.
5. **Concluir o engine DDS-first.** Criar writer persistente no construtor, introduzir
   `ProviderConstraint` tipado e conectar a configuração/CLI. Fortalecer o teste para
   múltiplas inferências e um único writer.
6. **Validar em camadas.** Formatter e testes unitários; Clippy; suítes FFI/DDS estático;
   smoke público; Miri para trechos pure-Rust e sanitizador para FFI se o toolchain local
   suportar. O dev container é usado quando o host não possui dependências.
7. **Revisar e reconciliar.** Atualizar o relatório da fase, a matriz código↔dissertação,
   o estado das tasks e executar revisão final independente de objetivo, QA, qualidade,
   segurança e contexto.

## Decisões técnicas

- Preferir ownership interno (`Arc<OwnedEntity>` ou lifetime explícito) em vez de exigir
  disciplina do chamador para manter handles vivos.
- Uma API que aceita handle/type raw só permanece segura se a compatibilidade for
  comprovada por tipos; caso contrário vira `unsafe fn` com seção `# Safety`.
- APIs de filtro dinâmico que contrariem o contrato de thread-safety do CycloneDDS serão
  restringidas ao estado anterior à criação de endpoints ou removidas da superfície
  segura; não será criado um fallback que esconda risco de UAF.
- `ProviderConstraint` converte explicitamente para os três literals IDL; o engine local
  não reutiliza a semântica ambígua de `ANY` como default.
- Testes DDS usam domínio até 232 e execução serial quando compartilharem descoberta.

## Gates

- **Gate A — biblioteca pura:** derive/codegen, CDR e testes de contratos sem FFI.
- **Gate B — biblioteca FFI:** suíte `cyclonedds`/`cyclonedds-test-suite` com
  `CYCLONEDDS_STATIC=1` e Clippy.
- **Gate C — runtime:** agente + dataspace com `--features dds`, contrato IDL e smoke.
- **Gate D — segurança:** Miri onde FFI não participa; ASan/TSan ou justificativa
  verificável para as lacunas de cobertura FFI.
- **Gate E — fidelidade:** nenhuma afirmação da matriz excede a evidência observada.
