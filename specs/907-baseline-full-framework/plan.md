# Plano — 907: Baselines full-framework no repo

## Como

Pacote Python `baselines/py/baselines/` (precedente de Python no repo: `workers/laya-worker`),
vendored do harness EXP0 (`benchmarks/orchestration/src/bench/`) e estendido:

| Módulo | Origem | Mudança |
|---|---|---|
| `contracts.py` | vendado | intacto (Schema v1) |
| `assembly.py` | vendado | intacto |
| `prompts/*.txt`, `fixtures/cases.jsonl` | vendados | intactos (hashes do freeze 860 conferidos no teste) |
| `providers/deterministic.py` | vendado | intacto — sustenta os testes byte-idênticos |
| `providers/llama.py` | **novo** | `LlamaClient` OpenAI-compat (`complete`/`acomplete`/`astream_events` opcional), temp/seed/max_tokens, timeout, erro tipado |
| `systems/langgraph/{seq,forkjoin}.py` | adaptado | client injetável (não só Deterministic), checkpointer explícito, `RetryPolicy`, modo stream |
| `systems/maf/{seq,forkjoin}.py` | adaptado | idem + checkpoint/resume explícito, `run_stream` |
| `systems/dds_rust.py` | **novo** | driver do lado DDS: HTTP `/api/v1/chat/completions/sync` (T1–T6) + `submit-one`; células 1 e 2 agentes |
| `scenarios/*.py` | **novo** | seq, forkjoin, serial, recovery, retry, concurrency, stream — assinatura única `(system, client, workload) → RunTrace` |
| `driver.py` | **novo** | CLI: `--system {dds,maf,langgraph} --scenario --runs --warmup --llama-url --model --seed --out`; interleave round-robin; manifest |
| `analysis.py` | **novo** | percentis, TOST IC90, Hodges–Lehmann; MESMAS margens da 880 (MEIs Tab. 7); referência: `specs/880-campanha/analise/` |

## Mapeamento para os experimentos 880

| Experimento 880 | Cenário 907 | Nota |
|---|---|---|
| EXP0 (byte-idêntico) | testes pytest com client determinístico | paridade estrutural contínua no repo |
| EXP1/1a/1b (overhead) | `seq` com backend comum + células DDS | T_extra = T_total − T_backend |
| EXP2 (LLM real) | `seq`/`forkjoin` com `LlamaClient` | mesmo modelo nos 3 |
| EXP3 (escala) | `concurrency` c∈{1,2,4} | makespan/p95 |
| EXP4 (recuperação) | `recovery` fail-once + retomada | DDS: célula reaper/reassign; MAF: checkpoint; LG: checkpointer |
| — (novo) | `stream` (TTFC) | única função sem cobertura 880 |

## Ambiente

- venv Python 3.10 em FS local (`~/.cache/tese-venv-907`): `langgraph==1.2.11`,
  `agent-framework==1.18.0`, `httpx`, `pytest`, `scipy` (TOST). Reusar o venv do piloto
  (`~/.cache/tese-benchmarks/agentboard/.venv`) se os pins conferirem; senão, novo venv
  + `requirements-frozen.txt`.
- llama-server real (default `http://127.0.0.1:8083`, gemma-4-12b; `-np 1 -c 32768
  --temp 0 --jinja` — regras SMOKE §0b do B-BENCH como referência operacional).
- DDS: domínio dedicado (79), bins com `CYCLONEDDS_STATIC=1`; em SMB,
  `CARGO_TARGET_DIR` em FS local.

## Riscos

- APIs de streaming do workflow (LG/MAF) podem não expor chunk a chunk → REQ-007 permite
  PARCIAL com evidência.
- LLM real não garante byte-idêntico → REQ-014 (estrutural + hash de request).
- Server np=1 serializa inferência → concorrência medida é de orquestração (igual aos 3).
- Branch atual carrega T905/T906 em voo → nenhum arquivo de runtime é tocado.
