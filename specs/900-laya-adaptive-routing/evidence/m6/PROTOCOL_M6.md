# Protocolo M6 (freeze pré-campanha) — estudo COMPLEMENTAR (não-RQ5)

Congelado em 2026-10-17 ANTES de qualquer run (regra do test-plan: metas e
desenho definidos antes da coleta; nenhum valor presumido).

## Braços
1. `static/rules` — roteamento fixo atual (baseline de regras).
2. `laya` — decisões do Laya (adapter real, checkpoint pinado por hash).
3. `planner-only` — sem Laya, planner LLM direto para workload complexo.
4. `laya+planner` — decisão Laya + planner quando needs_planning.

## Controles por run (manifest obrigatório)
- git HEAD, Cargo.lock hash, IDL hash, cyclonedds/lighthouse versions,
  model/checkpoint SHA-256, hardware (CPU/GPU/RAM), domínio DDS, config
  loopback, N, warmup, timestamp, operador.

## Amostragem
- N >= 30 runs independentes por braço (mesma unidade da dissertação);
  ordem aleatória de braços; 5 runs de warmup descartados por braço;
  outliers: MAD > 3.5 reportados em tabela própria (nunca removidos).

## Métricas (dicionário da tese: TTFC/ICL em chunks, relógios)
- Latência de decisão (Laya/planner) separada de inferência/DDS/claim.
- Makespan; P50/P95/P99; throughput; sucesso/erro tipados; recuperações.
- Roteamento correto vs dataset rotulado (macro-F1 por capability);
  calibração (ECE/Brier) SÓ se houver modelo real calibrado.
- Custos: tokens por etapa; microunidades por etapa (tabela versionada).

## Ameaças à validade (declaradas antes)
- Dataset seed rotulado pela IA executora (engenharia) — NÃO calibration-grade.
- Sem calibração validada, decisões Laya permanecem shadow (gate desligado).
- Comparação é estudo complementar; não altera RQ1–RQ4 nem adiciona RQ5.

## Critérios de parada/rollback
- Erro tipado > 5% dos runs em um braço → suspender admissão, drenar,
  reconciliar IN_DOUBT, registrar causa; sem migração implícita de tasks.
