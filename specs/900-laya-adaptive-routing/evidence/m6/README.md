# Evidências M6 — canary/protocolo (T-900-M6-01..03)

Data: 2026-10-17.

## M6-01 canary opt-in — CONFIG CONGELADA (sem execução)

Rollback/admissão já implementáveis com o que existe:
- Entrada do modo adaptativo é OPT-IN por processo (flags `--adaptive*`,
  domínio dedicado) — desligar = não iniciar/reiniciar sem as flags; nenhum
  estado migrou para o legado (tasks laya1- só existem no domínio dedicado).
- Drain: engine para de materializar novos nós (ready set congela); trabalhos
  em voo seguem o protocolo de claim legado até terminal; journal conserva
  IN_DOUBT para reconciliação (nada é repetido automaticamente).

## M6-02 protocolo complementar — CONGELADO ANTES DA CAMPANHA

Documento: `PROTOCOL_M6.md` (neste diretório). Braços: static/rules × Laya ×
planner × Laya+planner; N≥30/run por braço (mesma unidade experimental da
dissertação), warmup 5 descartados, outliers por MAD>3.5 reportados (não
removidos), métricas: TTFC/ICL, makespan, custo por etapa (Laya/planner/DDS/
claim/inferência separados), P50/P95/P99; sucesso/erro/recuperação; ameaças à
validade (dataset seed rotulado por engenharia; sem calibração não há gate de
confiança ativo). **Nenhum número de desempenho é presumido.**

## M6-03 runs/manifests — **BLOCKED**

Depende de: pin de SDK/checkpoint Laya (R-LAYA — decisão do usuário),
hardware de inferência e autorização de campanha. Nenhum run foi executado;
nenhuma métrica coletada. Como o M5-outbox→DDS e o planner LLM real também
dependem desse ambiente, ficam na mesma fila de desbloqueio.
