# REPORT CONSOLIDADO — Fase 880: Campanha confirmatória (2026-10-05)

**Estado: 6 de 8 experimentos executados com dados; 2 pendências de execução/protocolo (abaixo).** Runtime de referência: tag `congelamento-pre-campanha` @ `853c968` (+fases 820/830 aplicadas no branch). Hosts do laboratório em uso (.61 RTX 3080 / .62 orquestrador / .63 cliente / .64 agentes).

| Experimento | Estado | Resultado-chave | Artefato |
|---|---|---|---|
| EXP0 equivalência funcional | ✅ **PASSOU** | 85 fontes (17/caso), 0 conflitos, BYTE-IDÊNTICO entre MAF/LangGraph/DDS (+pilotos set/2026) | `exp0/` |
| EXP1 overhead (n=30×2 cond + controle) | ✅ **MEDIDO** | T_extra: controle 2–3 ms · LG ≈60 ms · MAF ≈207 ms · **DDS ≈770 ms** — estável entre condições ✓ | `exp1/RESULTS.md` |
| EXP1a ablação substrato | ✅ **MEDIDO** | DDS ≡ InMemory: p50 18×18 ms (delay 0), 162×164 (delay 50) → **custo do substrato ≈ 0** | `exp1a/RESULTS.md` |
| EXP1b ablação coordenação | ✅ **MEDIDO** | claim ↔ despacho central: **Δ ≤ 1 ms → equivalentes** | `exp1b` (dados em exp1b/) |
| EXP2 LLM real | ✅ **MEDIDO** | 30/30, T_total p50 5313 ms (3×~1,78 s reais na RTX 3080); T_extra ≈ 14,5% do total | `exp2/RESULTS.md` |
| EXP3 escalabilidade c∈{1,2,4,8} | ✅ **MEDIDO** | 450 workflows, 0 falhas; p50 +2,1% em c=8; vazão 0,6→4,1 wf/s; **E(8)=0,85** | `exp3/RESULTS.md` |
| EXP-C contenção k∈{2,4,8} | ✅ **MEDIDO** | 600 tasks, **0 duplicatas**; claims perdidos 3%→86,5% com k (sinal de contenção); skew de proximidade de rede (.61 96% em k=8) | `expc/RESULTS.md` |
| EXP4 falha/recuperação | ◐ **piloto + 30 runs parciais** | Recuperação rápida 663-667 ms (3/5 piloto); **2 P1 novos**: (a) agente congelado = starvation (owner 100 sobrevive ao lease); (b) cauda >15 s — e a injeção em t+0,9 s errou a janela vulnerável nos 30 runs (matando após o estágio) | `exp4/PILOTO.md` + `definitivo/` |

## Respostas às questões (estado)

- **RQ2 (overhead, EXP1/1a/1b/2):** respondida com dados: T_extra decomposto — coordenação (3×250 ms confirmação) ≫ transporte (EXP1a: substrato ≈0; EXP1b: decisão ≈0); com LLM real a fração cai para ~14,5% (EXP2). **H1: fora da zona de equivalência** (DDS ≠ MAF em overhead, direção DDS>MAF) — TOST formal pendente no pacote final.
- **RQ3 (escalabilidade, EXP3):** curvas medidas; E(8)=0,85 (limítrofe vs MEI ±15%) — TOST formal pendente.
- **RQ4 (recuperação, EXP4):** parcial — DDS side: piloto + 30 runs com injeção a refinar (matar no claim observado); 2 P1 novos descobertos (starvation por agente congelado; cauda de recuperação). MAF side (checkpoint/retomada): não executado.

## Pendências da fase (próximo ciclo)
1. **EXP4 refinado:** kill sincronizado com o claim observado (log-driven), 30 runs, + lado MAF (harness python, fail-injection em C).
2. **Análise formal:** TOST/Hodges–Lehmann/Holm sobre os pares EXP1/EXP2/EXP3 (dados já coletados).
3. **P1s do EXP4:** desenhar mitigação para agente congelado (strength 200 transitório / unregister API / deadline-miss→FAILED) e instrumentar republisher.
4. EXP1a: investigar take/read split do `DataSpace` (questão de API P2, instrumento documentado).

## Notas de execução (honestidade)
- 1ª passada do oráculo EXP0 com glob errado — detectada e corrigida (17 fontes/caso).
- EXP4 definitivo 1ª rodada sem injeção de falha (kill omitido) — descartada e re-executada com injeção; injeção em tempo fixo (t+0,9 s) errou a janela vulnerável na maioria das reps — protocolo a refinar para log-driven.
- Mock com eprint temporário de diagnóstico (`REGRESSAO`) em `in_memory.rs` — remover antes do merge.
