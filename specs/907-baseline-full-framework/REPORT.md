# REPORT — Fase 907: Baselines full-framework + campanha (EM ANDAMENTO)

## Estado: T-907-01..11 concluídos; T-907-12 (campanha) em execução

## Entregue (commits e00d668 → dfe9493, branch `t907-baseline-parity`)

- **Spec SDD completa** (`spec.md` REQ-907-001..014 + matriz de paridade F1–F9, `plan.md`, `tasks.md`) alinhada ao congelamento 860 e à campanha 880 (que esta fase NÃO repete às cegas — traz os baselines para o repo e adiciona streaming).
- **`baselines/py/` autossuficiente no repo**: contracts/assembly/prompts/fixtures/workloads vendados byte-exatos (6 hashes 16-hex = freeze 860 conferidos por teste); providers deterministic + `LlamaClient` (OpenAI-compat, temp/seed/max_tokens, erros tipados, SSE); porta uniforme `LLMPort` + envelopes Faulty/CallCounter/Timing; adapters **LangGraph 1.2.11** e **MAF 1.18.0 full-framework** (seq, forkjoin, checkpointer/`RetryPolicy`, checkpoint/resume MAF com seleção causal do checkpoint de B — padrão EXP4/880); driver DDS via bin `submit-one` (encadeamento no harness, gather p/ fork-join) + cliente `/sync` T1–T6 (sem content, documentado).
- **Cenários**: seq, forkjoin, serial, recovery (LG retomada via checkpointer; MAF via checkpoint), retry (`RetryPolicy` nativa LG; re-execução de executor MAF — análogo documentado, não idêntico), stream (**granularidade honesta**: token no direct/SSE; *superstep* em LG/MAF — PARCIAL por design, nunca simulado; REQ-907-007), concorrência c-workflows (LG/MAF in-process; DDS via gather de submit-one — claim disputado entre agentes).
- **Driver de campanha**: interleave round-robin entre sistemas, warmup marcado, seed por run, manifest (SHA do gguf, versões, commit), JSONL por sistema; **analysis**: percentis, TOST pareado IC90, Hodges–Lehmann, MEI Tab. 7 (variance-zero tratado).
- **Testes**: 35 passed + 1 skip (smoke DDS real exige bins+agentes no domínio).

## Piloto (T-907-11) — LLM REAL, mesmo modelo nos 3 sistemas

- Modelo: `gemma-4-12b-it-UD-Q4_K_XL.gguf` — **SHA-256 `90fd944d…f6f8e` = exatamente o modelo congelado do PILOTO 3-SISTEMAS (B-BENCH)**; llama-server 0.7.0 HIP local :8090, `-np 1 -c 16384 --temp 0 --jinja`, `max_tokens 1024`.
- DDS: 2 agentes (`pilot-a/pilot-b`) dominio 79, `--engine http`, claim disputado real.
- **seq** (n=3+warmup, caso-04): 12/12 completos; **conteúdos byte-idênticos entre os 3 sistemas** (temp 0); p50 LG 17,26 s / MAF 17,27 s / DDS 17,21 s.
- **forkjoin** (n=3+warmup, caso-02): 9/9 completos; conteúdos idênticos; p50 27,07–27,25 s.
- TOST demonstrado no piloto (DDS vs MAF): veredicto formal produzido (Δ −50 ms, DDS mais rápido) — **n=3 é indicação, não estatística**; campanha n≥30 decide.

## Rodando agora (T-907-12)

EXP1 seq n=30+warmup10 × 3 sistemas → EXP2 forkjoin idem → EXP3 concorrência c=4 (DDS incluído, claim disputado) → EXP4 recovery n=30 (LG+MAF; **DDS-EXP4 = protocolo 880 congelado, não re-executado** — requer orchestrator/reaper + injeção forçada; célula suportada, decisão documentada). Resultados em `~/.cache/tese-results-907/campaign-*/` (fora do repo, REQ-907-013).

## Desvios honestos

- Granularidade de streaming em LG/MAF = superstep (evidência: APIs `astream`/`run(stream=True)` expõem eventos de executor; token-level só no server/DDS) — REQ-007 cumprido como PARCIAL documentado.
- EXP4 lado DDS não re-executado nesta fase (ver acima); 880 EXP4 (30+30, p50 14,95 s vs 382 ms) permanece a evidência canônica.
- EXP3 com célula única c=4 (c=1 ≙ EXP1; 880 já cobriu c∈{1,2,4,8} no runtime congelado).
