# T904 — Plano de implementação e rollback

## Ordem

| Marcos | Conteúdo | Gate |
|---|---|---|
| M0 ✅ | Auditoria somente leitura (audit.md, registry, evidências) | matriz completa; T900 hash EXATO ⇒ segue |
| M1 | Esta spec (spec/architecture/plan/tasks/risks/traceability/README) | revisão antes do 1º código funcional |
| M2 | Testes FALHANDO do adapter (fixtures pequenos, loader injetável) | falham pela causa esperada |
| M3 | adapter.py/cli.py + tools/assemble_checkpoint.py; montagem real do T900; smoke de carga | pytest verde; smoke carrega 35813054…; hash divergente falha controlada |
| M4 | Manifesto de calibração (v1–v4=BASE; T900 sem calibração) ⇒ **SHADOW_ONLY**; opção de recalibração com custo registrado | calibração rastreável OU SHADOW_ONLY explícito |
| M5 | Shadow REAL: worker T900 + wf-observe em domínio isolado (7 cenários) | tasks_seen=0, tasks_written=0, causas tipadas |
| M6 | E2E ativo — **condicionado à calibração aceita**; sem ela: BLOCKED/NOT_RUN com evidência | verde OU bloqueio específico documentado |
| M7 | Regressão: cargo test (4 crates), clippy, fmt, pytest; DDS real em target dir exclusivo | verde; exceções = bloqueio real |
| M8 | Docs: README worker, doc workflow-engine, REPORT/REPRODUCE/FINAL_HANDOFF, correções datadas | outra IA reproduz com o handoff |

## Rollback

- Mudanças de código confinadas a adapter.py/cli.py/testes + 1 ferramenta NOVA.
- Rollback = reverter os 2 arquivos editados + apagar ferramenta/pasta de montagem;
  nada de IDL/Rust/contrato é tocado, então o rollback não afeta outros subsistemas.
- A montagem é symlink-only: remover o diretório `~/.cache/tese-laya/checkpoints/…` não toca
  nos artefatos canônicos T900–T903.
- Modo legado (`RealLayaAdapter()` sem path) preservado ⇒ experimentos do BASE seguem reproduzíveis.

## Pontos de parada (herdados do prompt, §8)

T900 ausente/hash divergente (M0: PASS), SDK não carrega formato local, calibração não ligável
(esperado ⇒ SHADOW_ONLY + M6 bloqueado), shadow criar qualquer Task, alteração de tópicos,
dados selados para ajuste, custo de GPU não autorizado, risco de sobrescrever pesos.
