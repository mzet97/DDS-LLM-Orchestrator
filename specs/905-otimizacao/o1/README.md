# O-1 — Streaming DDS nativo com gemma4 (2026-10-18)

## Estado: CONCLUÍDO — pipeline streaming ponta a ponta + paridade 20/20; TTFC tem DUAS semânticas (veredicto abaixo)

## O que foi feito

1. **Rebuild do fork** `third_party/llama.cpp_dds` @ `sync/upstream-20261018` (HEAD `6ea8665b8`)
   com `-DLLAMA_DDS=ON -DGGML_HIP=ON -DAMDGPU_TARGETS=gfx1100` (ROCm 7.1 do host,
   clang do ROCm como `CMAKE_HIP_COMPILER` — CMake 4.4 rejeita o wrapper `hipcc`),
   build dir off-SMB `/var/tmp/llama-o1-build`. Resultado: **llama-server b11759**
   com arch `gemma4` + hooks `--enable-dds/--dds-domain` (o build-dds antigo era
   b8114 pré-gemma4). O pipeline de streaming já existia no código (ponte C++
   publica chunks `is_final=false` com `seq_num`; `DdsEngine` Rust consome).
2. **`submit-one --stream`** (commit junto com a Fase R) + **`ttfc_ms` no
   `TaskResult`** do cliente (latência do primeiro chunk observado).
3. **Patch de paridade no bridge** (`tools/server/server.cpp`, fork): chunks DDS
   agora carregam apenas `content_delta` da maquinaria `task_result_state::
   update_chat_msg` — a MESMA do endpoint HTTP. Sem isto, o gemma4 publicava o
   canal de raciocínio `<|channel>thought` DENTRO do content (paridade 0/20).
   Fallback para templates sem split preservado.

## Bugs de configuração descobertos no caminho (documentar no runbook)

- **`CYCLONEDDS_URI` global do host** (`~/.config/cyclonedds/cyclonedds.xml`,
  peers do cluster) era herdado pelo servidor C++ quando iniciado sem override —
  processos com configs diferentes NÃO se descobrem (sintoma: request publicado,
  nada chega). Fix: config explícita **igual para todos** os processos do teste
  (`autodetermine` interfaces; ver `o1_measure.py`/runbook).
- **Filtro de modelo silencioso do bridge** (`DDSBridge::handle_request`): compara
  `request.model_name` com `model_loaded_` e descarta divergentes **sem log** (o
  print é `#ifdef DDS_DEBUG`). `-m models/gemma-…` (caminho relativo) ≠ task
  `gemma-4-12b…` → todos os requests dropados. Fix operacional: **`--alias` igual
  ao model_name das tasks**.
- Re-entrega TransientLocal: quando o matching finalmente acontece, o reader puxa
  o histórico KeepLast(10) do writer e enfileira TUDO no slot único (np=1) —
  requests antigos "saram" da fila minutos depois (respostas chegam sem leitor).

## Números (RX 7900 XTX, domínio 79 loopback-host, temp 0, max_tokens 2048)

| cenário | n | TTFC p50 | total p50 | TTFC % |
|---|---|---|---|---|
| gemma4 DDS stream (pipeline bruto, pré-patch split) | 20/20 | **443 ms** | 8 247 ms | **6,0%** |
| gemma4 DDS stream (content limpo, pós-patch) | 20/20 | 7 043 ms | 8 027 ms | 89,5% |
| gemma4 HTTP não-stream (HttpEngine) | 20/20 | 7 664 ms (=total) | 7 664 ms | 100% |
| Phi-4-mini DDS stream (content, não-reasoning) | 10/10 | **275 ms** | 607 ms | **45%** |

- **Paridade de conteúdo gemma4: 20/20 byte-a-byte** (DDS stream × HttpEngine).
- **Veredicto TTFC:** o TRANSPORTE entrega chunks incrementais rápido (443 ms
  brutos = 6% do total; Phi 275 ms de content). O TTFC-*content* do gemma4 é
  89,5% porque o modelo PENSA ~90% do tempo antes da resposta — intrínseco ao
  modelo, não ao DDS. A métrica correta por cenário depende do consumidor: UIs
  que exibem raciocínio têm TTFC ~6%; consumidores de resposta final não ganham
  TTFC com modelos think-first (mas ganham entrega incremental de raciocínio).
- **Nondeterminismo Phi controlado:** DDS×HTTP 5/10 idênticos E DDS×DDS 5/10 —
  a divergência é da GERAÇÃO (greedy em GPU não é bit-determinístico entre runs
  para o Phi), não do mapeamento do bridge (gemma4 20/20 idêntico).

## Artefatos

`o1_measure.py` (harness), `o1_{stream,nostream}20*.jsonl` (runs gemma4),
`o1_phi_{stream,nostream}.jsonl`. Runbook: servidor com `--alias` + config DDS
`autodetermine` uniforme + agente `--engine dds`; builds em `/var/tmp`
(CARGO_TARGET_DIR `/var/tmp/tese-target-bbench`).
