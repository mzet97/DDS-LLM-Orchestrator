# T908 — Unified Closure: relatório integral da campanha

**Período:** 2026-11-03 a 2026-11-04 | **Executor:** IA (SDD evidence-first) | **Autor:** Matheus Zeitune
**Status final:** `BLOCKED_WITH_TRAINED_CANDIDATE` (finalista melhor modelo da linhagem; 8/15 gates; promoção é decisão do autor)

---

## 1. Missão e contrato

Fechar todas as competências acima de 0.80 com checkpoint treinado, seguindo o prompt mestre
T908 (M0 a M11): auditar T907, corrigir dados, novo sealed OOD com quotas, gates
pré-registrados antes do treino, treino de 3 braços, seleção no dev, calibração, abertura
única do sealed, avaliação pareada dos 4 modelos (B0/N2/C2/T908) e decisão honesta.

Contrato: `QUESTIONS_V1` rev 1 (12 perguntas imutáveis: qos_profile 5 classes, needs_planning,
complexity 0-3, urgency 0-2, 8 capabilities booleanas).

Gates pré-registrados (`manifests/gates_preregistration.json`, congelados ANTES de qualquer
treino): QoS macro ≥0.80 e por classe ≥0.80; STREAM P/R/F1 ≥0.80; planning acc ≥0.90 e F1 das
duas classes ≥0.80; complexity macro ≥0.80 e por nível ≥0.80 com suporte mínimo 30; urgency
macro e por nível ≥0.80; caps macro e por capability ≥0.80 com ≥30 positivos; sem regressão de
security_audit abaixo de 0.76; guardrail de recall BALANCED ≥0.80; dominância vs C2 sem
regressão >0.01 em nenhum agregado; promoção nunca automática; rollback = T904.

---

## 2. Cronologia real da campanha

**2026-11-03 (madrugada/tarde):**
- M0: atlas de erros do sealed T907 (`t908/reports/m0_error_atlas.json`): CRITICAL→BALANCED
  22/73 (30%), urgency 1→0 48/188, planning missed 53 vs 5 false, complexity nível 3 com zero
  suporte no sealed (bug do gerador T907: k∈{1,2,3} só gera cx 0/1), security_audit 19 FN
  (C2 regrediu 0.761→0.524), summarization 0.712.
- M4: gates congelados + dossiê commitado (ce806bc).
- Suplemento r1: 330 textos gerados na .61, ouro por construção (k_stages, banco de prazos,
  cenários de caps) + filtro de concordância LLM: 200 mantidos para o pool.
- Autor pediu pausa para mover o servidor fisicamente (calor); "mata tudo no .61" executado
  por PID explícito; estado salvo em `reports/PAUSE_RESUME_2026-11-03.md`.

**2026-11-03 (noite) e 2026-11-04 (dia): servidor de volta em 192.168.1.x**
- Setup de treino LOCAL na .61 (plano Colab descartado: o sandbox local mata processos longos,
  o upload de 1.6 GB nunca completava): venv `~/venv-t908` com torch 2.14.1+cu126 (o PyPI
  default instala torch cu132 e o driver 535.309 só suporta CUDA 12.2) + laya 0.4.10; CUDA True.
- Checkpoints C2 e B0 transferidos via scp com SHA verificado
  (`624de57e…` e `bda679e7…`), montados em `/tmp/t908_init/{c2,b0}` por symlink + complementos.
- Pipeline de dados em cadeia automatizada (master térmico com rajadas): gen sealed (165 lotes,
  495 textos, domínios inéditos), labels, rodada 2 de suplemento (48 lotes S1+S2) e labels.
- Incidente térmico longo: a 3080 no novo local operava cronicamente a 92°C com colapso de
  clock (915→450 MHz). Diagnóstico: placa no alvo térmico de fábrica (83°C) com curva de fan
  conservadora; `nvidia-smi -r` cura o estado de clock preso; tampa aberta + ajustes do autor
  (fan 100%, resfriamento) destravaram a placa para 73-87°C com clock cheio (1575-1920 MHz).
- Autor corrigiu a refrigeração fisicamente; a partir daí treino e geração rodaram a pleno.

**2026-11-04 (tarde/noite):**
- Top-up dirigido do sealed em 4 fases após auditoria de cobertura pré-congelamento:
  A caps (90 lotes), B qos+urgência (45), C complexidade k∈{1,5,6,7,8} (30), R research-only
  (30), Q qos 5 classes (100). Emenda M3 commitada (e39f7c3) ANTES dos congelamentos: ouro de
  complexity/needs_planning do sealed = medido por LLM (yield 495/495 na base); qos/urgência/caps
  permanecem por construção com filtro.
- Split final: pool de treino 5.527 linhas (leak check 8-gram bidirecional: 776 drops no pool,
  120 nos candidatos sealed); dev 223/232; cal 117/198.
- Treino dos 3 braços na 3080 (4 épocas, soft-ce, micro-batch 8, seed 908, ~40 min/braço com a
  refrigeração corrigida): a=init C2 (`bced0c97…`, dev 0.960), b=init C2 com suplemento
  oversampled (`3f1d2179…`, 0.956), c=init B0 controle (`c3f07d70…`, 0.868). H2 confirmada de
  novo: init do finalista anterior supera init do baseline.
- M6 seleção no dev: **braço a**.
- Calibração de temperatura no split cal na .61: pin `194133d9…` (198 pares,
  temperaturas [1.428, 2.240, 1.456]).
- Aberturas do sealed: v1 ABORTADA (composição sem STREAM/LOW_COST/FAILOVER: o grid original
  só tinha CRITICAL/BALANCED), v2 ABORTADA (truncamento do corte[:460] consumiu as quotas de
  qos; quotas somavam 590 > 460 slots). Correções: grid S1 estendido para 5 classes, topQ (100
  lotes) e sealed de 590 linhas. Nenhum treino ou promoção usou as aberturas abortadas.
- **v3 aberta uma única vez (13:04 UTC)**: 590 linhas, qos 380/90/40/40/40, caps ≥31,
  complexity níveis 30/243/280/37, urgency 457/68/65.
- Avaliação pareada dos 4 modelos (CPU, avaliador único T907-M1-v1, predições por exemplo).
- DECISION_T908.json + entrega `models/laya_t908_unified/` + atualização de notes.md.

---

## 3. Dados

| Conjunto | Conteúdo | Observação |
|---|---|---|
| Pool de treino | 5.527 linhas = T907 (5.237) + r1 (200) + r2 (90, filtro estrito) | CRITICAL 48 e urgency 1/2 = 43/45 reforçados pelos contrastivos |
| Suplemento r1 | 330 textos; 200 mantidos | eixos do atlas: CRITICAL×BALANCED, urgência 0/1/2, cx por k, security_audit implícito |
| Suplemento r2 | 144 textos; ~90 mantidos | S1+S2 (qos/urgência) |
| Sealed v3 | 590 textos + labels, abertura única | qos 380/90/40/40/40; caps todas ≥31; cx níveis 30/243/280/37; urg 457/68/65 |
| Leak control | 8-gram bidirecional | pool vs sealeds anteriores: 776 drops; candidatos sealed vs pool+sealeds: 120 drops |

Arquivos de splits em `t908/datasets/splits/` (train/dev/calibration + `split_manifest.json`
com SHAs) e sealed em `t908/datasets/sealed_export/` (pool `f1cbae25…`, labels `df5df836…`,
`SEALED_OPEN_PROTOCOL.json` com o registro das 3 versões).

---

## 4. Treinos

| Braço | Init | SHA-256 (model.safetensors) | Dev composto | Observação |
|---|---|---|---|---|
| a (finalista) | T906 C2 `624de57e…` | `bced0c972336182470ac104eeeeb0ae9c90a404e6c3152add58e92f1810d18b3` | 0.960 | treino normal |
| b | T906 C2 | `3f1d2179a204967bb4b2dc0ae79e0e6a875057252ae00bdf746407af027883c9` | 0.956 | suplemento oversampled ×3 |
| c | T904 B0 `bda679e7…` | `c3f07d703297baba94b095ab1b87cd596f9b7f56d59ce7e61c79b776ef34bf61` | 0.868 | controle (confirma H2: init do finalista anterior > init do baseline) |

Config: 4 épocas, soft-ce, micro-batch 8 × grad-accum 8, encoder-lr 2.5e-5, head-lr 1e-4,
seed 908, device cuda (RTX 3080 da .61). Dados: `/tmp/t908_data/` (não sobrevive a reboot).
Relatórios de treino em `t908/models/arm_{a,b,c}/train_report.json` (dev held-out: acurácia
0.977→0.984 no braço a; ECE 0.019→0.010).

---

## 5. Sealed T908 v3 e avaliação pareada (n=590, abertura única em 2026-11-04T13:04 UTC)

SHAs: pool `f1cbae25e425275c212f55046ed7cb484c96e50ff7419c35e8e4eb2d08410d79`,
labels `df5df8363937abaf81b52ed45d84f001e2e0b1286974022f54b64a4f7dc1dc0a`,
eval mesclado `a48493eb9b90059176401ddf6a575dd55850cd981784949bd33d914e4b08cfaf`.

Resultados (qos macro / stream F1 / urgency macro / planning acc / complexity macro / caps macro / security_audit F1):

| Modelo | qos | stream | urgency | planning | complexity | caps | security_audit | gates |
|---|---|---|---|---|---|---|---|---|
| **T908-a** (+calib) | **0.8818** | 0.9189 | **0.4903** | **0.9644** | **0.5654** | **0.8271** | **0.9577** | **8/15** |
| C2 (vigente) | 0.8747 | **0.9620** | 0.4423 | 0.9610 | 0.3550 | 0.7210 | 0.3913 | 7/15 |
| N2 | 0.8654 | 0.9487 | 0.3998 | 0.9356 | 0.4545 | 0.8178 | 0.7419 | 7/15 |
| B0 (T904) | 0.7770 | 0.8539 | 0.3085 | 0.9627 | 0.2731 | 0.8242 | 0.8947 | 6/15 |

QoS por classe do finalista: BALANCED 0.933, STREAM 0.919 (P=1.00, R=0.85), LOW_COST 0.827,
FAILOVER 0.935, **CRITICAL 0.795 (recall 0.711)**: 4 de 5 classes ≥0.80.

### Gates do finalista (8 PASS / 7 FAIL)

PASS: qos_macro 0.8818; stream_p 1.00; stream_r 0.85; stream_f1 0.9189;
planning_accuracy 0.9644; caps_macro 0.8271; security_audit_no_regression 0.9577;
balanced_recall_guardrail 0.9737.

FAIL: qos por classe (CRITICAL 0.795 < 0.80); planning_f1 da classe False (52 negativos,
ruído alto); complexity_macro 0.5654 (confusão 2→1 em 241/280 no OOD); urgency_macro 0.4903
(gold 0 → pred 1 em 210/457); desdobramentos por nível/capabilidade derivadas desses.

### Dominância vs C2

Melhorias: complexity +0.2104, caps +0.1061, urgency +0.0480, planning +0.0034, qos +0.0071.
Regressão: stream −0.0431 (única; excede a margem de 0.01 do pré-registro).

---

## 6. Decisão

`BLOCKED_WITH_TRAINED_CANDIDATE`: o finalista é o modelo mais capaz da linhagem em OOD
(domina 5 de 6 agregados e não regride em security_audit), mas 7 gates seguem abertos e há
uma regressão de stream. A promoção é decisão exclusiva do autor; o vigente permanece T904
(`bda679e7…`, laya-production).

Opções registradas no `DECISION_T908.json`:
1. Manter T904/B0 e tratar T908-a como candidato de pesquisa;
2. Promover T908-a condicionado ao M8 (shadow DDS) + M9 (benchmarks), aceitando a regressão
  de stream em troca dos ganhos;
3. T909: replay balanceado de urgency/complexity OOD + hard-negatives de stream.

---

## 7. Causas raiz abertas (para T909)

1. Urgency: over-predição de nível 1 em texto OOD (210/457 erros gold 0); nível 2 comporta bem.
2. Complexity: nível 2 → 1 em 241/280 casos OOD; nível 3 ok quando presente (37 no sealed).
3. QoS CRITICAL: recall 0.711 no OOD (22/90 critical→balanced), apesar dos contrastivos.
4. Stream: o finalista perde para C2/N2 (sinal de stream em domínio novo; C2 foi treinado com
  mais STREAM no pool T906).

---

## 8. Incidentes, causas e lições

1. **Refrigeração da 3080 no novo local**: 92°C crônico com colapso de clock (915→450 MHz);
  `nvidia-smi -r` cura o estado de clock preso; abrir a tampa + fan 100% + limite 280W
  (ajuste do autor) resolveram. Placa opera no alvo térmico de fábrica (83°C) com curva
  conservadora: não é defeito, é fluxo de ar.
2. **pkill/pgrep por padrão mata o próprio SSH** quando o padrão casa com o comando (ocorreu
  3×): matar por PID via `ps -eo pid,args | grep | awk -v m=$$ '$1!=m'`.
3. **torch cu126 vs cu132**: PyPI instala torch cu132; o driver 535.309 só suporta CUDA 12.2;
  usar `--index-url https://download.pytorch.org/whl/cu126`.
4. **gemma morto por 11h**: morto intencionalmente para liberar VRAM no treino e não relançado;
  o gen em execução passou 11h re-tentando um servidor morto. Sempre checar
  `curl /health` após qualquer janela de treino.
5. **Quotas de sealed maiores que os slots** (590 > 460): o truncamento `[:n]` cortava a cauda
  (quota de qos). Solução: sealed de 590 com quotas que fecham na soma.
6. **/tmp da .61 não sobrevive a reboot**; dados de treino e checkpoints de init em /tmp exigem
  re-envio após reboot.
7. **Sandbox local mata processos longos** (uploads Colab, nohup locais): cargas longas devem
  rodar detached na .61 (setsid + nohup + log em arquivo), nunca como processo local longo.
8. **relativo path do nohup**: 2º `cd ../../..` demais fez a cadeia v2 nascer no diretório
  errado e morrer; usar caminhos absolutos nos encadeamentos.
9. **Filtro de construção no sealed descartava 60%** (desacordo de complexity k→nível):
  emendado para ouro medido por LLM nessas dimensões (política T906/T907), com registro
  pré-congelamento (emenda M3, e39f7c3).

---

## 9. Artefatos

- Dossiê SDD: `src/rust/specs/908-laya-unified-closure/` (branch `t908-unified-closure`:
  ce806bc, e39f7c3): gates preregistrados, emenda M3, M0/M4 report, PAUSE_RESUME, lineage.
- Entrega: `src/laya_t900_handoff/t908/models/laya_t908_unified/` (model.safetensors
  `bced0c97…`, calibration_v1.json `194133d9…`, encoder/, tokenizer/, rl_agent_config.json,
  MANIFEST.json).
- Braços: `t908/models/arm_{a,b,c}/` (pesos + train_report.json + SHA).
- Relatórios: `t908/reports/` (m0_error_atlas, dev_t908_{a,b,c}, sealed_t908_{a,b0,n2,c2} com
  `_predictions.jsonl` por exemplo, gates_t908.json, SEALED_OPEN_PROTOCOL.json).
- Decisão: `t908/DECISION_T908.json`.
- Scripts: `t908/scripts/` (gen/label/build/split/prepare/evaluator/m6/m7/train_t908_61.sh).

---

## 10. Pendências

1. M8 shadow DDS + M9 benchmarks: pré-requisitos apenas se o autor promover o T908-a.
2. Revisão humana dos 300 casos (fila da triagem T905/T906): PENDING; concordância
  LLM×LLM não substitui revisão humana.
3. Decisão do autor entre as 3 opções da seção 6.
