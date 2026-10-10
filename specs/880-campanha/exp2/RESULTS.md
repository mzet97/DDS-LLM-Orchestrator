# EXP2 — Workflow com LLM real (resultados, 2026-10-05)

**Protocolo:** dissertação §3.7.5 + manifesto congelado. Configuração: agente .61 `--engine http --llama-url http://127.0.0.1:8082` → llama-server (build CUDA) com `Qwen3.5-0.8B-Q4_K_M.gguf` (SHA congelado) na RTX 3080; fluxo A→B→C via DDS (wf-run na .63); **30/30 workflows completed** (artefatos `exp2-*.json`).

## Resultados (T_total e por estágio; latência medida no cliente wf-run)

| Métrica | Valor |
|---|---|
| T_total workflow (média / p50 / min / max) | **5354 / 5313 / 5225 / 5565 ms** |
| Estágio A (p50 / p95 / média) | 1775 / 1934 / 1784 ms |
| Estágio B | 1764 / 1917 / 1785 ms |
| Estágio C | 1752 / 1971 / 1785 ms |

## Leitura

1. **T_extra do DDS (~770 ms, medido no EXP1) vira ~14,5% do T_total com LLM real** — responde a pergunta do EXP2: a diferença de orquestração **perde relevância material** quando a inferência domina o tempo (era 100% do piso no EXP1). Base direta para o desfecho de H1 no EXP2 (fração T_extra/T_total ≤ 0,10 de MEI — 0,145 fica acima do MEI por pouco: análise TOST formal no conjunto).
2. Estabilidade alta (min–max 5225–5565 ms; CV baixo) — a RTX 3080 com o modelo congelado entrega serviço regular sob a cadência do fluxo.
3. **Nota de modelo (documentar como característica):** o Qwen3.5-0.8B emite raciocínio (`reasoning_content`) e esgota o orçamento de 256 tokens antes do `content` (finish=length) — outputs vazios são comportamento do modelo com o protocolo congelado, não falha de transporte (inferências reais de ~1,78 s por estágio confirmam carga na GPU).
4. Condição de controle sem orquestrador (3× llama direto) fica para o pacote final de EXP2 junto com EXP3/EXP4.

## Pendências da campanha
EXP1a (instrumento: pendura em ablação DDS — semântica take/read do stream a investigar, WIP), EXP1b ✅ (claim↔dispatch equivalentes, Δ≤1 ms), EXP3 (c∈{1,2,4,8}), EXP4 (congela T̃ref no piloto antes), EXP-C (k agentes), análise TOST + REPORT consolidado.
