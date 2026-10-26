# Tasks — 907

Loop SDD (`../../AGENTS.md` §1): test-first, gates pytest + `cargo clippy -D warnings` +
`cargo fmt --check` (workspace inalterado ⇒ gates de cargo só no fim, como regressão),
commit `[T-907-XX]` por task.

- [x] T-907-01 Bootstrap: esta spec + `baselines/README.md` + branch. **Aceite:** arquivos existem, branch criada, README declara os diferenciais (REQ-012).
- [ ] T-907-02 Vendor: `contracts.py`, `assembly.py`, `providers/deterministic.py`, `prompts/*.txt`, `fixtures/cases.jsonl` + esqueleto de pacote + pytest de fumaça (import + hashes dos prompts = freeze 860). **Aceite:** `pytest baselines/py` verde (REQ-001).
- [ ] T-907-03 `providers/llama.py` `LlamaClient` (sync/async/stream opcional, temp/seed/max_tokens, timeout, erro tipado) + testes contra o backend determinístico como mock HTTP. **Aceite:** teste de requisição/resposta + erro tipado (REQ-008).
- [ ] T-907-04 Adapters LangGraph full: `systems/langgraph/{seq,forkjoin}.py` com client injetável, checkpointer explícito, `RetryPolicy`. **Aceite:** paridade seq/forkjoin com MAF e direct via client determinístico (REQ-002/003).
- [ ] T-907-05 Adapters MAF full: `systems/maf/{seq,forkjoin}.py` com client injetável, checkpoint/resume explícito. **Aceite:** mesma paridade (REQ-002/003).
- [ ] T-907-06 Cenários recovery + retry (fault 1× em C / transiente) nos 2 frameworks + contagem de chamadas por estágio. **Aceite:** retomada sem reexecução de A/B; rastro de tentativas (REQ-004/005).
- [ ] T-907-07 Cenário stream (TTFC) nos 2 frameworks + DDS (`TaskOutput` chunks); PARCIAL documentado se API não expuser. **Aceite:** TTFC medido de evento nativo ou PARCIAL com evidência (REQ-007).
- [ ] T-907-08 Cenário concurrency c∈{1,2,4} com isolamento por sessão + teste anti-cross-talk. **Aceite:** zero cross-talk, métricas por workflow (REQ-006).
- [ ] T-907-09 `systems/dds_rust.py`: HTTP sync (T1–T6) + submit-one; células 1 e 2 agentes; smoke com llama real (skip sem server, padrão do EXP0). **Aceite:** run DDS e2e verde com server no ar; skip limpo sem (REQ-009).
- [ ] T-907-10 `driver.py` (CLI, interleave, warmup, manifest, JSONL) + `analysis.py` (percentis, TOST IC90, Hodges–Lehmann, MEIs Tab. 7) + testes com fixtures sintéticas. **Aceite:** análise reproduz veredicto em fixture conhecida (REQ-010/011).
- [ ] T-907-11 Piloto smoke n=2–3 por célula com LLM real → ajustes + REPORT parcial. **Aceite:** 3 sistemas × cenários executados; manifest completo (REQ-008/009/013).
- [ ] T-907-12 **Campanha EXP1–EXP4 n≥30** intercalada → `benchmarks/orchestration/results/<run_id>/`. **Aceite:** ≥30 runs/condição, sanity em todo JSONL, manifest por run (REQ-010/013).
- [ ] T-907-13 REPORT.md da fase + `notes.md` (raiz) + ponteiros AGENTS.md com data. **Aceite:** números medidos vs MEIs citados; afirmações só com TOST (REQ-011).
