# Matriz de capacidades dos geradores — T-900-M1-01 (2026-10-16/17)

Geradores: **idlc C 11.0.1** (`~/.local/cyclonedds/bin/idlc`), **idlpy Python 11.0.1**
(backend `_idlpy` compilado do sdist em venv 3.13) e **cyclonedds-build 3.0.1**
(parser interno; o `build.rs` do dds-contract usa `try_idlc: false`, ou seja, só ele).

| Constructo | idlc C 11.0.1 | idlpy 11.0.1 | cyclonedds-build 3.0.1 | Consequência no IDL |
|---|---|---|---|---|
| `string<N>` (bound) | ✅ preserva bound no tipo C/CDR | ✅ `types.bounded_str[N]` | ⚠️ **descarta bound** → `DdsString` (codegen.rs: "Bounded strings use DdsString in the current derive macro") | Mantido no IDL (C/Python honram); divergência de TypeId XTypes Rust é risco aberto R-INTEROP para M1-02/M1-04 |
| `sequence<T,N>` | ✅ | ✅ `types.sequence[T,N]` | ✅ `DdsBoundedSequence<T,N>` | OK |
| `sequence<string<N>,M>` | ✅ | ✅ `sequence[bounded_str[N],M]` | ⚠️ `DdsBoundedSequence<DdsString,M>` (bound interno perdido) | idem string |
| `@key` (múltiplas, string/u32) | ✅ 2 chaves no descritor | ✅ `annotate.key(...)` | ✅ `#[key]` | OK |
| `@final` em struct | ✅ (sem warning de extensibilidade) | ✅ `@annotate.final` | ⚠️ aceito e ignorado | Aplicado em TODAS as structs |
| Default de membro `= 1` | ❌ `syntax error` (linha do campo) | ❌ (mesmo parser do idlc) | ❌ `IDL parse error: Expected Semi, got Assign` | **Removido do IDL**; `schema_version=1` é regra de contrato/validação (fixture + receptor) |
| Literais de enum duplicados no módulo | ❌ `Declaration 'CANCELLED' collides with an earlier declaration of 'CANCELLED'` | ❌ (mesmo parser) | ⚠️ **leniente** (aceitou) | **EMENDA M1-01**: literais prefixados (IC_/QI_/DS_/RC_/NK_/WS_/NS_), ordinais intactos |
| `module` / enums / primitivas u16..u64/i32/f64/bool | ✅ | ✅ | ✅ | OK |
| Opção `-o <dir>` | ✅ | ⚠️ **ignora** — escreve `./<modulo>/` no CWD | ✅ | M1-02 deve copiar do CWD, não confiar em `-o` no backend py |

## Ambiente Python — como o backend `_idlpy` foi obtido

1. Wheel 11.0.1 do site do usuário (py3.14) **não traz `_idlpy`**: `idlc -l py`
   falha em `cyclonedds/__idlc__.py` (`IndexError` no glob `_idlpy*`);
   `idlc` chega a imprimir `cannot load generator py` e **ainda assim exita 0**
   sem gerar nada — armadilha registrada.
2. Sdist `cyclonedds-11.0.1.tar.gz` (pip download, rede disponível nesta sessão):
   `find_cyclonedds()` do setup.py só acha a biblioteca do idlc se ela estiver em
   `lib/`; a instalação local do usuário tem `libcycloneddsidl.so` **em `lib64/`**
   → sem `_idlpy`. Solução: `CYCLONEDDS_HOME` sombra com `lib/` unificada
   (symlinks) em /tmp; rebuild → `_idlpy.cpython-313-x86_64-linux-gnu.so`.
3. Sdist em **Python 3.14 quebra no import** (`annotations.py` antigo:
   `Member TRY_CONSTRUCT1 is not defined` — PEP 649/ordem de anotações; o wheel
   do usuário tem um `annotations.py` corrigido que funciona no 3.14, mas sem
   `_idlpy`). Venv com **Python 3.13** resolve (instalação sdist íntegra).
4. `idlc -l py` resolve o backend executando `python3 -m cyclonedds.__idlc__`
   (via `sh -c`): o `PATH` decide qual instalacao Python é usada.

Decisão para M1-02: fixar a receita acima (sdist + py3.13 + home sombra) ou
gerar bindings com pin próprio no worker; documentar no pyproject do
`workers/laya-worker`. Nada foi instalado no ambiente do usuário.

## Consequências para a proposta M0-1 (contracts.md)

- **EMENDA 1 — prefixos de enum**: literais ganham prefixo por regra de escopo
  IDL (idlc C rejeita duplicatas no módulo). Ordinais/semântica exatamente como
  contracts.md; os 5 tópicos, chaves, ordem de campos, bounds e tipos seguem a
  proposta sem alteração.
- **EMENDA 2 — sem default `= 1`**: IDL 4.2 não suporta; ambos os geradores
  rejeitam. `schema_version` carrega valor obrigatório 1 em V1 garantido por
  validação (fixture congela a regra; receptores rejeitam versão desconhecida).
- **RISCO ABERTO — bound de string no Rust**: se M1-02 integrar o IDL ao
  build.rs como está, os tipos Rust anunciarão strings NÃO limitadas no XTypes
  enquanto C/Python anunciam limitadas → mismatch de TypeId no matching
  (V02/M1-04 deve provar/comparar). Alternativas a decidir em M1-02:
  pós-processar o gerado Rust, validar bounds só na aplicação, ou revisar o
  contrato (reduzir bounds de string). Não decidido nesta tarefa.
