# Evidências M2 — worker Laya em shadow (T-900-M2-01..04)

Data: 2026-10-17 (UTC). M1 aceito (sessão autorizou "fazer todos os specs").

## Entregas

| Tarefa | Arquivos | Aceite |
|---|---|---|
| M2-01 worker package | `workers/laya-worker/{pyproject.toml,src/laya_worker/{__init__,cli,space}.py}` | CLI DDS (`--mock`), adapter mockável; **24 unit tests** (unittest, stdlib) sem pesos/rede/download; `RealLayaAdapter` exige pin de checkpoint e levanta causa tipada sem importar SDK |
| M2-02 perguntas+parser+validator | `src/laya_worker/{questions,adapter,decide}.py` + crate **`workflow-engine`** (`decision.rs`, `capability_catalog.rs`) | QUESTIONS_V1 versionada; decide() com correlação/validação local/erros tipados (RC_*)/payload_hash; **validator Rust: 12 testes** da matriz REQ-900-004; catálogo V1 (8 rótulos) par com o worker |
| M2-03 dataset+baseline+métricas | `dataset/dataset_v1_seed.jsonl` (32 PT/EN) + `tools/evaluate.py` | splits fixos (18/14); baseline regras no TEST: qos acc **0.857**, needs_planning **1.0**, capability macro-F1@0.5 **0.552**, cobertura 1.0 (por construção). Relatório: [rules_baseline_seed_report.json](rules_baseline_seed_report.json) |
| M2-04 shadow real (V04) | `harness/run_shadow.py` + bin `wf-observe` | **PASS**: 3 intents → worker 3 decisões (tasks_written=0) → observador 3 ACCEPT (shadow_only=3), **tasks_seen=0**. Log: [shadow-run.log](shadow-run.log) |

## Gates

- `cargo test -p workflow-engine`: **14/0** (12 validator + 2 catálogo); clippy
  `-D warnings` OK (mock e dds); fmt OK.
- `cargo build -p workflow-engine --features dds`: `wf-observe` OK.
- Worker: `python3 -m unittest discover`: **24 OK** (adaptador fake determinístico,
  self-consistency com NaN/soma/domínio, RealAdapter gate, digest parity 16/16
  vetores golden, decide matrix incl. erros RC_* e determinismo de hash).
- Cross-language runtime: o `payload_hash` do worker usa o MESMO encoder do
  contrato (16 vetores reproduzidos em Python E Rust).

## Descobertas da sessão shadow (ajustes de contrato em vida, sem mudar schema)

1. Intenções do ensaio NÃO pinam `checkpoint_revision` (pin vazio): decisão de
   regras não pode alegar checkpoint do Laya — validator rejeita divergence
   (observado ao vivo e corrigido no fixture do harness, não no contrato).
2. Timestamps toy (`deadline_ns=2`) fazem a decisão expirar instantaneamente —
   validação de janela funcionou como especificado; harness usa `time_ns()`.

## BLOCKED / pendências honestas

- **R-LAYA segue BLOCKED p/ inferência real**: SDK ausente, versão/licença/
  checkpoint/hardware não pinados pelo usuário. `RealLayaAdapter.decide()`
  levanta `LayaUnavailableError`; nenhum peso baixado em teste.
- Dataset é SEED de engenharia (rótulo da IA executora), não
  calibration-grade; ECE/Brier e thresholds congelados dependem de modelo
  real + dataset rotulado adequado (M6 decide).
- Calibration_revision vazia → todo Accept é `shadow_only` (correto: sem
  calibração válida, permanece shadow).
