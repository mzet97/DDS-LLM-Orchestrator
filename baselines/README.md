# baselines/ — LangGraph + MAF full-framework × DDS-LLM-Orchestrator

Baselines **canônicos no repositório** para a comparação da dissertação:
os frameworks **completos** (`langgraph` 1.2.11, `agent-framework` 1.18.0 — Python de
nascença, conforme a nota de método da campanha 880) contra o orquestrador DDS-Rust,
com o MESMO LLM real, as MESMAS mensagens e a MESMA instrumentação.

Spec: [`specs/907-baseline-full-framework/`](../specs/907-baseline-full-framework/spec.md) ·
Protocolo: [`specs/860-backends/manifest-freeze.md`](../specs/860-backends/manifest-freeze.md)
· Campanha executada: [`specs/880-campanha/`](../specs/880-campanha/REPORT.md).

## Como rodar

```bash
# venv (FS local — SMB não suporta symlinks de venv)
python3.10 -m venv ~/.cache/tese-venv-907
~/.cache/tese-venv-907/bin/pip install -r baselines/py/requirements-frozen.txt

# fumaça (sem LLM real — backend determinístico)
~/.cache/tese-venv-907/bin/python -m pytest baselines/py -q

# run com LLM real (3 sistemas recebem o MESMO modelo)
~/.cache/tese-venv-907/bin/python -m baselines.driver \
    --system {langgraph|maf|dds} --scenario seq --runs 30 --warmup 10 \
    --llama-url http://127.0.0.1:8083 --model gemma-4-12b \
    --out ~/.cache/tese-results-907/<run_id>/
```

Lado DDS usa os bins existentes SEM modificá-los (`orchestrator` HTTP sync com T1–T6,
`agent --engine http`, `client/submit-one`; build `CYCLONEDDS_STATIC=1`, domínio dedicado).

## Matriz de paridade (resumo — detalhe e aceites na spec)

| Função do orquestrador | LangGraph | MAF | Status |
|---|---|---|---|
| Workflow seq A→B→C | `StateGraph` | `WorkflowBuilder` | PAR |
| Fan-out/fan-in + serial | ramos + `max_concurrency` | fan-out/fan-in edges | PAR |
| Recuperação fail-once + retomada | checkpointer + `thread_id` | checkpoint/resume | PAR |
| Retry com teto | `RetryPolicy` | re-execução do executor | PAR (semântica análoga, documentada) |
| Concorrência c∈{1,2,4} | grafos paralelos | executores concorrentes | PAR |
| Streaming (TTFC) | `astream` | `run_stream` | PAR ou PARCIAL com evidência — nunca simulado |
| Contexto entre estágios | estado do grafo | shared state | PAR |

## Diferenciais do DDS-LLM-Orchestrator (sem equivalente nativo nos baselines)

Estas funções **não existem** nos frameworks de baseline e por isso NÃO são simuladas
neles — são exercitadas só no lado DDS e mensuradas como custo/benefício:

- **Claim distribuído data-centric** (agentes reivindicam `PENDING` via DDS; sem
  despachante central) — célula com 2 agentes em claim dispute.
- **Seleção de agente** por health/slots/especialização (`select_agent`).
- **QoS dinâmica** (loop NFCM/Zadeh, knobs por task, `QoS.Metric/Violation`).
- **Recuperação no data space** (reaper → reatribuição → claim por sobrevivente),
  distinta da retomada por checkpoint que MAF/LangGraph fazem **no cliente** —
  comensurável só como "workflow completou + custo", não como mecanismo (lição EXP4/880).

## Honestidade

- Com LLM real a equivalência é **estrutural** (mesmas mensagens/ordem/modelo — hash de
  request); byte-idêntico só com o backend determinístico (testes do EXP0 vendidos aqui).
- Resultados de run ficam FORA do repo (`benchmarks/orchestration/results/` ou
  `~/.cache/tese-results-907/`); nenhum artefato é commitado neste diretório.
- Nenhuma afirmação de superioridade sem TOST (α=0,05, IC 90%, MEIs da Tab. 7).
