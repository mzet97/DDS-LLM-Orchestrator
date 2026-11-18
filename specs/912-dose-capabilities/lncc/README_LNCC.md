# T912 no supercomputador SDumont (LNCC)

Treino dos braços C0 (Qwen3.5-2B) e C1 (Qwen3.5-0.8B) da campanha T912
(dose por classe + 12 perguntas) em nós GPU NVIDIA do SDumont, com os mesmos
hiperparâmetros e versões de pacotes provados localmente na RX 7900 XTX.

Contexto: alocação do orientador (proposta SD n. 249768, UERJ, programa
Standard). Repo público: `github.com/mzet97/DDS-LLM-Orchestrator`, branch
`t912-dose-capabilities`.

## Arquivos

| Arquivo | Onde roda | Função |
|---|---|---|
| `sdumont_prepare.sh` | login node (1×) | clone + venv + pip + download dos modelos HF no scratch |
| `sdumont_train.sbatch` | via `sbatch` | job SLURM: 1 GPU, chama `t912_run_all.sh` |
| `t912_run_all.sh` | dentro do job (ou local) | cadeia completa: smoke → C0 → C1 → dev → seleção/calibração → sealed+gates → tar |
| `requirements-t912.txt` | (usado pelo prepare) | versões pinadas = ambiente provado da RX |

## Passo a passo

1. **Acesso** (fora da LNCC, precisa VPN da LNCC ou rede acadêmica):

   ```bash
   ssh SEU_USUARIO@login.sdumont.lncc.br
   ```

2. **Preparar** (uma vez, no login node; tem internet):

   ```bash
   # traga o pacote (se o repo já não estiver lá):
   mkdir -p $SCRATCH/t912
   cd $SCRATCH/t912
   git clone -b t912-dose-capabilities https://github.com/mzet97/DDS-LLM-Orchestrator.git repo
   bash repo/src/rust/specs/912-dose-capabilities/lncc/sdumont_prepare.sh
   ```

   O prepare cria `$SCRATCH/t912/{repo,venv,hf-cache}`. Tudo no scratch porque
   nós de computação não leem `/prj`.

3. **Submeter**:

   ```bash
   cd $SCRATCH/t912/repo/src/rust/specs/912-dose-capabilities
   sbatch lncc/sdumont_train.sbatch
   ```

   Antes de submeter pela primeira vez, confira as filas disponíveis com
   `sinfo -s` e, se necessário, edite `#SBATCH -p` no sbatch (default
   `sequana_gpu_prod`, nós 4× V100 32GB; a fila `hpc3_gpu_prod` tem H100 80GB
   se o projeto tiver acesso; `sequana_gpu_dev` serve para teste rápido).

4. **Monitorar**:

   ```bash
   squeue -u $USER
   tail -f results/infra/train_chain.log      # progresso da cadeia
   cat slurm-t912-<jobid>.out                 # saída do job
   ```

5. **Trazer os resultados de volta** (após o job terminar): o job gera
   `results/t912_sdumont_<data>.tar.gz` com modelos (C0, C1, finalista
   calibrado), métricas, predições por exemplo, gates e protocolo do sealed:

   ```bash
   # na sua máquina:
   scp SEU_USUARIO@login.sdumont.lncc.br:$SCRATCH/t912/repo/src/rust/specs/912-dose-capabilities/results/t912_sdumont_*.tar.gz .
   ```

## O que a cadeia executa (ordem)

1. **preflight**: verifica CUDA/GPU e registra device, compute capability e bf16.
2. **smoke** (`SMOKE=1` default): treino de época-0 do 0.8B em diretório
   separado; valida modelo+dataset+save+predict antes de gastar tempo real.
3. **treino C0** (2B) e **C1** (0.8B): LoRA r16 α16, 3 épocas, lr 2e-4 cosine,
   mb 16 (`MB` ajustável), seed 909, checkpoints por época com SHA-256.
   Em V100 (sem bf16) o unsloth cai para fp16 automaticamente.
4. **dev eval** (n=1000) de cada braço com predições por exemplo.
5. **seleção** por min-F1 nas 22 classes (empate: caps_pos_macro, depois custo).
6. **calibração** do finalista em CAL (n=700).
7. **sealed** (`RUN_SEALED=1` default): abertura única (open_count 0→1) e
   avaliação pareada C0/C1 no sealed n=2000 com IC95 bootstrap + gates do
   PREREG_T912. Para adiar a abertura: `RUN_SEALED=0 sbatch ...`.
8. **tar** com tudo para scp.

Idempotente: braços com `train_report.json` e evals com `metrics.json` são
pulados; um job re-submetido após falha retoma de onde parou.

## Também funciona local (RX)

A mesma cadeia roda na RX (ROCm) sem SLURM:

```bash
PY=~/.cache/tese-laya/venv-t909-rocm/bin/python \
T912_WORK=~/.cache/tese-laya/t912_train \
RUN_SEALED=0 bash lncc/t912_run_all.sh
```

## Problemas conhecidos

| Sintoma | Causa provável | Correção |
|---|---|---|
| preflight "CUDA indisponível" | driver antigo p/ o wheel escolhido | rerodar prepare com `TORCH_CUDA=cu128`; pior caso refazer venv com torch mais antigo |
| `module: command not found` no login | shell sem modules | `source /etc/profile.d/modules.sh` (ou zsh: veja FAQ SDumont) |
| fila rejeita o job | nome de fila mudou / projeto sem cota GPU | `sinfo -s`, ajustar `#SBATCH -p`; confirmar cota GPU da alocação (a tabela da proposta 249768 pediu CPU+GPU=0 UA; GPU NVIDIA aparece só no texto) |
| download HF falha no prepare | proxy/rede da LNCC | usar `HF_HUB_ENABLE_HF_TRANSFER=0`; se persistir, baixar na sua máquina e `rsync` para `$SCRATCH/t912/hf-cache` |
| OOM no treino | VRAM menor que 32GB | `MB=8 sbatch ...` (registrado no train_report) |
| job passa de 8h | evals lentos em V100 | re-submeter (retoma); ou `--time=24:00:00` (limite da fila `sequana_gpu` é 96h) |

## Custo (UA)

Nós GPU têm contabilização diferenciada (fila `gdl` ~2,0 UA/h nos docs
públicos; filas `sequana_gpu_*` ~1,5). O job completo cabe com folga em 8h de
1 GPU. **Confirme com o orientador a cota GPU da alocação antes de rodar a
campanha inteira.**

## Protocolo científico

O sealed (pool+labels+protocolo) está congelado neste repo com SHAs no
`PREREG_T912.json` e `datasets/MANIFEST.json`. A abertura é única e só ocorre
após o freeze de pesos+calibrador (etapa 7 acima). O PREREG exige que o commit
de freeze preceda o treino: o commit que adiciona este pacote É o freeze.
