# T908 — Estado na PAUSA (2026-11-03 ~15:20 local)

**ATUALIZAÇÃO ~18:30: servidor de volta. Setup .61 reconstruído e pipeline no ar:**
- venv `~/venv-t908` com torch 2.14.1+cu126 (CUDA True; PyPI default é cu132 e o driver 535
  só suporta CUDA 12.2 — usar `--index-url https://download.pytorch.org/whl/cu126`) + laya 0.4.10
- C2/B0 em `/tmp/t908_init_{c2,b0}.safetensors` (SHA verificado) montados em
  `/tmp/t908_init/{c2,b0}/` (symlinks) com complementos em `/tmp/t908_comps/`
- gemma :8083 no ar; **master térmico `/tmp/master_t908_61.sh`** (rajadas de 420s esperando
  temp<72°C) encadeia: sealed gen 190 → sealed label → r2 gen 48 → r2 label → MASTER_DONE
- **Problema físico aberto: a 3080 no novo local não sustenta clocks** (bursts a ~90 t/s por
  ~40 min, depois colapso a 450 MHz a 92°C; reset `nvidia-smi -r` cura o estado preso, mas a
  causa é refrigeração). Autor avisado; fan/heatsink a checar. Plano B documentado: gerar no
  Muse local (desvio de professor, precisa de aval do autor).
- Progresso ~20:20 UTC: 129/190 lotes do sealed.

## O que estava rodando e onde parou

| Item | Estado na pausa | Sobrevive ao desligamento? |
|---|---|---|
| Suplemento T908 (gen+label, 330 textos) | ✅ CONCLUÍDO, já no pool local | sim (local + .61 /tmp) |
| Pool T908 (`t908/datasets/unified_pool_t908.jsonl`) | ✅ construído (5.237 + 200 supl.; rodada 2 vai re-construir) | sim |
| Sealed gen (190 lotes, seed 9081, domínios novos) | ⏸️ 30/190 lotes (117 textos) | SIM (arquivo na VM /tmp/laya_t908_sealed/, resumível por batch) |
| Sealed label | ⏳ não iniciado | — |
| Rodada 2 suplemento (48 lotes S1+S2, seed 9082) | ⏳ não iniciado (chain3 esperava o sealed) | — |
| Colab laya908 | ✅ PARADA (saldo 424,40 CU; gasto total 0,19) | n/a |
| Treino na .61 (novo plano) | ⏳ pip laya instalando em background na .61 | conferir ao voltar |

## Plano de treino MUDOU (pausa mostrou o caminho)

Upload Colab do sandbox local morre para processos longos ⇒ **treino será na .61**
(RTX 3080 10GB), NÃO no Colab. Sequência ao voltar:

1. **Relançar chain2 na .61** (sealed gen resume + label):
   ```bash
   sshpass -p 'Admin@123' ssh agent@192.168.1.61 'setsid nohup /tmp/t908_chain2.sh > /dev/null 2>&1 < /dev/null &'
   ```
   (chain3 também: `setsid nohup /tmp/t908_chain3.sh ... &` — espera SEALED_ALL_DONE no log)
2. Conferir pip laya na .61: `python3 -c "import laya"` (se faltou, `pip3 install --user --no-cache-dir laya==0.4.10`)
3. Após SEALED + R2 ALL_DONE no /tmp/t908_chain.log: **parar gemma** (libera VRAM; PID em `pgrep -f llama-server`),
   scp C2+B0 p/ .61 (~800 MB cada, LAN), rodar `laya-train` braços a/b/c sequenciais
   (se OOM em micro-batch 8, usar `--micro-batch 4 --grad-accum 16`).
4. Local: fetch gen/label da .61 → `build_t908_pool.py --pairs /tmp/laya_t908_sup,/tmp/laya_t908_r2` →
   `split_t908.py` → `prepare_train_t908.py` → treinos → `m6_select.py` → calibração →
   `m7_sealed_eval.py open/eval/gates` → DECISION_T908.json.

## Artefatos já garantidos (locais)

- `t908/reports/m0_error_atlas.json` (M0) + spec/908 commitada (branch t908-unified-closure, ce806bc)
- Gates congelados: `specs/908-laya-unified-closure/manifests/gates_preregistration.json`
- Suplemento r1: `/tmp/laya_t908_sup/{gen,label}_t908.jsonl` (local E na .61)
- notes.md §I.10 atualizado com a entrada 2026-11-03 (1)

## Regras que NÃO mudam

- Sealed T908 novo: abertura única; gates de fechamento 0.80 por competência
- Nenhuma promoção automática; rollback = T904 `bda679e7…`
- Colab só se .61 não der conta (saldo 424,40 CU, teto gasto T908 = 15 CU)
