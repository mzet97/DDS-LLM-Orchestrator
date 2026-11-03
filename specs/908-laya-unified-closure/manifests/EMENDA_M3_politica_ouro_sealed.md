# T908 — Emenda M3 (2026-11-03 ~18:15): política de ouro do sealed

**Contexto.** A auditoria pré-congelamento (feita com os artefatos já fetchados) revelou que a
política original do sealed (ouro de complexity/needs_planning POR CONSTRUÇÃO a partir de
k_stages com filtro de concordância) descartava ~60% dos candidatos (295/495 por divergência
de complexity) e deixava quotas inatingíveis: research=0 (células S4 na cauda do grid
interrompido em 165 lotes), QI_CRITICAL=3, complexity nível 2=3, urgency 2=3. Congelar assim
reproduziria o erro documentado da T905 (caps F1=0 por 0 positivos ⇒ gate inatingível por
design).

**Emenda (antes de qualquer congelamento/treino):**
1. Ouro de `complexity` e `needs_planning` no sealed = MEDIDO POR LLM (gemma-4-12b, temp 0,
   política v5), mesma precedência dos sealeds T906/T907. Sem filtro de construção nessas duas
   dimensões (yield 495/495).
2. Mantidos os filtros de construção onde a construção é o alvo do gate: qos (células S1) e
   urgency (células S2). Caps permanecem por construção.
3. Top-up dirigido do sealed (master2_t908_topup.sh), AINDA sob o mesmo contrato QUESTIONS_V1
   e antes do congelamento:
   - Fase A: S4_caps (todas as 15 combinações, 90 lotes, seed 9083)
   - Fase B: S1_qos_contrast + S2_urgency (45 lotes, seed 9084)
   - Fase C: S3_complexity com k∈{1,5,6,7,8} (30 lotes, seed 9085)
   - Rotulagem das 3 fases com o mesmo rotulador (resumível).
4. O split (`split_t908.py`) aceita múltiplos diretórios de candidatos sealed
   (`--sealed-dirs`), aplica leak check 8-gram bidirecional a TODOS, e seleciona por quotas
   (caps ≥30 pos, qos ≥40/classe, cx níveis com suporte ≥30, urg 0/1/2 com quotas) do pool
   combinado base+top-up.
5. O gasto orçamentário não muda (tudo na .61, 0 CU de Colab).

**Rastreabilidade:** esta emenda é registrada ANTES do congelamento do sealed e antes de
qualquer treino; os gates pré-registrados (`manifests/gates_preregistration.json`) NÃO mudam.
