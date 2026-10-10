# MANIFESTO DE CONGELAMENTO — pré-campanha confirmatória (T-860-02)

**Data do congelamento:** 2026-10-05 · **Válido para:** EXP0–EXP4, EXP1a/1b, EXP-C (`PLANO_IMPLEMENTACAO_RESTANTE.md` Fase 880)
**Regra:** qualquer mudança nos itens abaixo **invalida** o congelamento e exige novo manifesto + nova tag.

## Runtimes (commits imutáveis)

| Componente | Commit/SHA |
|---|---|
| Runtime Rust (`src/rust`) | `853c9688e789fff0fa407f9e82bd928a343baaec` (branch `studio/phase-800-node`, pós-fase 850) |
| Ponte C++ (`third_party/llama.cpp_dds`) | `940e70a702806f96413df5b665cb3b9ff1d317e3` |
| Binding DDS | crates.io `cyclonedds =3.0.1` / `cyclonedds-rust-sys =1.2.1` (pin exato, gate `t809`) |

## Modelo e artefatos congelados (SHA-256)

| Artefato | SHA-256 (ou 16 hex) |
|---|---|
| `models/Qwen3.5-0.8B-Q4_K_M.gguf` | `bd258782e35f7f458f8aced1adc053e6e92e89bc735ba3be89d38a06121dc517` |
| prompts `seq_A_analyst_v1.txt` | `52bc20d062f49b18` |
| prompts `seq_B_reviewer_v1.txt` | `6427c642776f0d82` |
| prompts `seq_C_consolidator_v1.txt` | `021b0e28c85452ec` |
| prompts `fork_A_correctness_v1.txt` | `99a1377689aaab48` |
| prompts `fork_B_security_v1.txt` | `a9aeac0c0c22a6e8` |
| prompts `fork_C_consolidator_v1.txt` | `999f1fe40283de4b` |
| workload `seq_chain_v1.yaml` | `82ee25305aa000a2` |
| workload `fork_join_v1.yaml` | `9e4ed5f6e6ed88cd` |
| workload `fork_join_serial_v1.yaml` | `18b0bf42f504e59a` |

## Equivalência temporal dos backends (T-860-01 — MEDIDA, destrava EXP1/EXP3)

- Fonte: `docs/equivalencia-backends.md` (regenerável por `bench.validation.calibracao`).
- Resultado (c=4, delay 50 ms, capacity 4, n=600 cada, warmup 20): serviço p50 **50,1 ms (HTTP) vs 51,2 ms (DDS)**; Δp50 = **1,1 ms** (critério ≤10); razão p95 = **1,030** (critério ≤1,2). Veredito: **EQUIVALENTES**.
- Divergência de saturação extrema permanece por design (429 imediato vs semáforo 30 s) — fora da região operacional da campanha (c ≤ 8 < capacity+queue).
- **Parâmetros congelados dos backends:** `delay_ms=50`, `capacity=4`, DDS `queue=64`, stub `semaphore=4` (sem timeout na região operacional); temperatura/max_tokens por workload YAML.

## Protocolo (aplicável a todo run)

- Unidade experimental = run independente; ≥30 runs/condição (EXP0: 5/sistema); warmup ≥10 descartadas (flag `warmup` por envio); aleatorização por blocos; limpeza de estado entre runs (reinício de processos); relógio monotônico local, correlação por `task_id`/`request_id`; **todo JSONL passa pelo sanity** (`bench.validation.sanity`) antes da análise.
- Análise: TOST (α=0,05, IC 90%), Hodges–Lehmann, Holm; razões em escala log; p99 exploratório.
- MEIs: Tabela 7 da dissertação (±20 ms; ±0,10 fração; ±15% vazão c=8; ±20% p95; H3 max(0,20·T̃ref, 500 ms) — **T̃ref a congelar no piloto independente antes de EXP4**).
- Dataset final: `campaign_data/` NÃO é fonte; runs novos com manifest por run.

## Tag

Tag anotada `congelamento-pre-campanha` no repo `src/rust` apontando para o commit acima. Recongelar = nova tag `congelamento-pre-campanha-N`.
