# M7 — Regressão (comandos e totais observados — 2026-10-21)

| Comando | Resultado |
|---|---|
| `cargo test -p dds-contract --locked` | ok (41 binários somados: **117 passed / 0 failed** nos 4 crates) |
| `cargo test -p workflow-engine --locked` | ok |
| `cargo test -p agent --locked` | ok |
| `cargo test -p dds-dataspace --locked` | ok |
| `cargo clippy --workspace --all-targets --all-features --locked -- -D warnings` (GATE DO CI, ci.yml:85) | **limpo** ("No issues found") |
| `cargo fmt --all -- --check` | limpo (exit 0) |
| `pytest workers/laya-worker/tests -q` | **34 passed** |
| re-hash dos 9 pesos | idênticos ao M0 (`m7-preservation-rehash.txt`) — nada sobrescrito |

## Observação pré-existente registrada (NÃO introduzida pela T904)

`cargo clippy --workspace --locked -- -D warnings` (default-features — combinação MAIS
ESTRITA que o CI) falha no HEAD `fc56e60`:

```
error[E0277]: the trait bound `dds_contract::workflow::dds_llm_workflow_v1::WorkflowIntent:
serde::de::DeserializeOwned` is not satisfied
error: could not compile `workflow-engine` (bin "dag_validate")
```

- Causa: `dag_validate` sem `dds` usa `serde_json::from_value::<WorkflowIntent>` sobre os
  tipos MOCK (com serde derives); sob unificação de features do workspace, `dds-contract`
  é compilado COM `dds` (tipos gerados por IDL, sem serde Deserialize) para todo o grafo —
  e o bin não-dds quebra.
- Prova de pré-existência: `git status -- '*.rs'` vazio (nenhum `.rs`/`Cargo.toml` alterado
  pela T904) ⇒ o estado compilado é exatamente o HEAD.
- Impacto CI: NENHUM — ci.yml:85 e security.yml:48 usam `--all-features`, que passa limpo.
- Ação sugerida (fora do escopo T904): `required-features`/cfg no bin ou feature `serde`
  dedicada no dds-contract para os tipos gerados.
