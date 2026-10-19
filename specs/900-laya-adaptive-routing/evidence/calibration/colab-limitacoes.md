# Limitações do Colab CLI para geração de dataset — 2026-10-17/18

## O que aconteceu

1. `colab new --gpu L4` cria a sessão com GPU alocada ✅
2. Upload/exec funcionam (caminho remoto ABSOLUTO é obrigatório — o
   relativo dá 500) ✅
3. Modelo baixado e pin verificado na VM ✅
4. **Descoberta crítica**: `libcuda.so.1` (driver) fica em
   `/usr/lib64-nvidia` — FORA do LD_LIBRARY_PATH padrão; sem isso o
   llama-server CUDA cai silenciosamente para CPU (0 MiB de GPU).
5. Processos detachados via `subprocess.Popen(start_new_session=True)`
   sobrevivem ao fim da célula ✅
6. **Limitação fundamental**: Colab **libera a GPU** (~90 min) quando não
   há browser conectado, mesmo com kernel BUSY executando o script. A
   sessão degrada para CPU e a geração (30 tok/s→0,5 tok/s) trava em
   PROG 0. O CLI `colab status/exec` NÃO conta como cliente browser.

## O que foi tentado

| Abordagem | Resultado |
|---|---|
| exec + thread no kernel | GPU ativa; sessão morre em ~1h sem ping |
| exec + keep-alive local (orchestrate_colab.py) | sessão ainda morre (GPU ≠ kernel) |
| `colab run` (script autocontido) | script executa, mas GPU liberada após ~40 min |
| `subprocess.Popen(start_new_session=True)` | processo sobrevive à célula; GPU não |

## Conclusão

Para jobs GPU > 1h no Colab é **obrigatório ter o browser aberto** no
notebook. O CLI serve bem para: transferir arquivos, executar setup,
validar pin, rodar jobs curtos (< 30 min de GPU).

**Caminho adotado**: geração LOCAL (RX 7900 XTX, 60 tok/s, comprovada em
M6) retomada com `--resume` até 2000; notebook interativo (célula pronta
em `COLAB_NOTEBOOK.md`) como opção complementar caso o usuário queira
gerar mais no Colab com browser aberto.
