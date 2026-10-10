# REPORT CONSOLIDADO — Fase 880: Campanha confirmatória (2026-10-06)

**Estado: 8 de 8 experimentos executados com dados; TOST formais EXP2/EXP3 concluídos (EXP1/TOST H1 já decidido em 2026-10-05).** Runtime de referência: tag `congelamento-pre-campanha` @ `853c968` (+fases 820/830 aplicadas; código de injeção forçada do EXP4: `25998a1`). Hosts do laboratório em uso (.61 RTX 3080 / .62 orquestrador / .63 cliente / .64 agentes).

| Experimento | Estado | Resultado-chave | Artefato |
|---|---|---|---|
| EXP0 equivalência funcional | ✅ **PASSOU** | 85 fontes (17/caso), 0 conflitos, BYTE-IDÊNTICO entre MAF/LangGraph/DDS (+pilotos set/2026) | `exp0/` |
| EXP1 overhead (n=30×2 cond + controle) | ✅ **MEDIDO + TOST** | T_extra: controle 2–3 ms · LG ≈60 ms · MAF ≈207 ms · **DDS ≈770 ms** — TOST H1: DDS−MAF Δ=563,6 ms, IC90 [548,6; 578,7] → **não-equivalência decidida** | `exp1/RESULTS.md` + `analise/tost_h1.py` |
| EXP1a ablação substrato | ✅ **MEDIDO** | DDS ≡ InMemory: p50 18×18 ms (delay 0), 162×164 (delay 50) → **custo do substrato ≈ 0** | `exp1a/RESULTS.md` |
| EXP1b ablação coordenação | ✅ **MEDIDO** | claim ↔ despacho central: **Δ ≤ 1 ms → equivalentes** | `exp1b` (dados em exp1b/) |
| EXP2 LLM real | ✅ **MEDIDO + TOST** | 30/30, T_total p50 5313 ms (3×~1,78 s reais na RTX 3080); fração T_extra/T_total = **0,1438**, Fieller IC90 [0,1411; 0,1466] ≡ bootstrap → **NÃO-equivalente decidida vs MEI 0,10** (desfecho informativo: overhead permanece material) | `exp2/RESULTS.md` + `analise/tost_exp2_exp3.py` |
| EXP3 escalabilidade c∈{1,2,4,8} | ✅ **MEDIDO + TOST** | 450 workflows, 0 falhas; modelo por bloco (excl. spawn do harness): **E(2)=0,995 · E(4)=0,975 · E(8)=0,920 → todos EQUIVALENTES** (IC90 dentro de [0,85;1,15]); ponto congelado c/ spawn cobrado: E(8)=0,854 (limite conservativo, na borda) | `exp3/RESULTS.md` + `analise/tost_exp2_exp3.py` |
| EXP-C contenção k∈{2,4,8} | ✅ **MEDIDO** | 600 tasks, **0 duplicatas**; claims perdidos 3%→86,5% com k (sinal de contenção); skew de proximidade de rede (.61 96% em k=8) | `expc/RESULTS.md` |
| EXP4 falha/recuperação | ✅ **MEDIDO (30+30)** | DDS: injeção forçada **30/30 mid-flight, 30/30 completados**; recuperação (estágio C) **p50 14.948,5 ms** (13.961–15.026; 0/30 >20 s) = lease DDSI; T̃ref ≈ 14,95 s → margem H3 ≈ 3,0 s. MAF: checkpoint/retomada **30/30, p50 382 ms** (381–397), sem reexecução de A/B (delta A:2 B:2 C:2) | `exp4/definitivo/RESULTS.md` + `exp4/maf/RESULTS.md` |

## Nota de método — papéis Rust × Python × C++ na campanha

Pergunta do autor ("por que Python e para quê?"): os 4 papéis são distintos e nenhum contamin a tese:
1. **Baselines MAF/LangGraph são Python de nascença** — o baseline é o framework exato; reimplementar em Rust invalidaria a comparação de orquestradores completos.
2. **Harness + análise em Python ÚNICA para os 3 sistemas** (backend determinístico compartilhado, oráculo, sanity, TOST com scipy) — elimina viés de ferramenta de medição; o protocolo congelado especificou "análise estatística em Python" antes da coleta (`analise/tost_h1.py`, `bench.validation.*`).
3. **Scripts inline de análise** (percentis, JSONL do EXP3/EXP-C) seguem a mesma stack dos artefatos.
4. **`dds_types.py`** existe pelo runtime legado Python (referência funcional) — o 19º tópico entrou em lockstep lá também (guarda de consistência, 18 tipos).
O sistema da tese **não é Python**: runtime/agentes/studio em Rust, ponte de inferência em C++. Defesa empírica: o EXP1a (ablação Rust-only, mesmo binário, Rust vs memória) mostra substrato DDS ≈ 0 (18×18 ms) — a camada Python de medição não contamina a conclusão. **Princípio: Rust = sistema; C++ = inferência; Python = baseline alheio + instrumento de comparação/análise.**

## Respostas às questões (estado final da campanha)

- **RQ2 (overhead, EXP1/1a/1b/2):** respondida com dados e estatística formal: T_extra decomposto — coordenação (3×250 ms confirmação) ≫ transporte (EXP1a: substrato ≈0; EXP1b: decisão ≈0); com LLM real a fração é **0,1438 (IC90 Fieller [0,1411; 0,1466]) vs MEI 0,10 → NÃO-equivalente decidida** (o overhead DDS permanece material mesmo com inferência dominante — desfecho informativo, não falha). **H1 (TOS primária): não-equivalência decidida (DDS > MAF, Δ=563,6 ms).**
- **RQ3 (escalabilidade, EXP3):** respondida: **E(2)=0,995, E(4)=0,975, E(8)=0,920 — equivalentes ao MEI ±15%** no modelo por bloco (sistema puro); com o spawn de processo/discovery do harness cobrado, E(8) cai para 0,854 (borda do MEI) — a degradação é do harness client-side, não do data space. Sem cliff nem colapso de cauda (p95 +9,7% em c=8).
- **RQ4 (recuperação, EXP4):** respondida com 30+30 runs: **recuperação FUNCIONA (30/30 DDS, 30/30 MAF)** com mecanismos não comensuráveis — DDS recupera **no data space** (reaper→reatribuição→claim por sobrevivente) a p50 **14,95 s**, custo dominado pelo lease DDSI de ownership sob kill -9 (relinquish só na expiração); MAF recupera **no driver** (checkpoint→retomada) a p50 **382 ms** (≈39× mais rápido, porém centralizado no cliente). T̃ref ≈ 14,95 s congelado → margem H3 ≈ 3,0 s. P1 residual honesto: agente CONGELADO (SIGSTOP, processo vivo) ainda starvation — mitigação (strength 200 transitório / unregister API / liveliness por task) documentada como trabalho futuro (muda protocolo, não bug).

## Pendências da fase (encerramento)
1. ~~EXP4 refinado/definitivo + lado MAF~~ ✅ concluído (esta atualização).
2. ~~Análise formal EXP2/EXP3~~ ✅ concluída (`analise/tost_exp2_exp3.py` + saída versionada).
3. **P1 do EXP4 (agente congelado)**: permanece como trabalho futuro documentado (PILOTO §2) — decisão de design de protocolo, fora do congelamento.
4. EXP1a: investigar take/read split do `DataSpace` (questão de API P2, instrumento documentado).
5. Holm/Hodges–Lehmann complementares sobre os pares EXP1: procedimento já especificado; se executado no pacote da dissertação, versionar em `analise/`.

## Notas de execução (honestidade)
- 1ª passada do oráculo EXP0 com glob errado — detectada e corrigida (17 fontes/caso).
- EXP4 definitivo 1ª rodada sem injeção de falha (kill omitido) — descartada e re-executada com injeção; injeção em tempo fixo (t+0,9 s) errou a janela vulnerável na maioria das reps → refinado log-driven (6/30 mid-flight) → **definitivo com injeção forçada via `target_agent` (30/30)** — commit `25998a1` (wf-run `--target-agent`, agente `--target-agent-prefix`, reaper limpa `target_agent` na reatribuição).
- Cauda aparente de 74,5 s no rep 2 do definitivo: ~59 s eram startup/descoberta do cliente `wf-run` (artefato do harness); a recuperação do estágio C foi 14.950 ms (dentro da distribuição) — métrica correta: latência do estágio C.
- Bootstrap E(c): 2 bugs de implementação corrigidos durante a análise (vazão por workflows vs blocos; divisor bloco/workflow) — versão final validada contra os pontos congelados do RESULTS.
- Mock com eprint temporário de diagnóstico (`REGRESSAO`) em `in_memory.rs` — ✅ removido (commit `9d39dea`).
