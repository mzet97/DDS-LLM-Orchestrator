# Catálogo de Figuras da Dissertação (verificado visualmente)

Conteúdo **real** de cada arquivo em `tese/69a588a60776208777b2007b/img/`, verificado
visualmente (todas as F1–F34). Serve de contexto visual da migração e de **auditoria** das
figuras da dissertação.

> ## ✅ CORREÇÃO APLICADA (2026-07-16)
> Os `\includegraphics` do `dissertacao.tex` foram corrigidos: cada figura de F19 em diante
> foi remapeada para o arquivo certo (F19→F20, …, F33→F34), de modo que **cada legenda agora
> aponta para a imagem correta**. A figura F34 do `.tex` (resultado E1, decomposição T1–T6)
> ficou **comentada com `% TODO-FIGURA-FALTANDO`** porque o gráfico não existe em `img/`
> (o `E1_latency_comparison_*.png` é latência mediana por protocolo, não a decomposição por
> camada) — legenda/label preservados (nenhum `\ref` quebrou). Backup:
> `dissertacao.tex.bak_figfix_20260716`. Diff = apenas linhas de figura.
>
> **`F23.png` foi INCLUÍDA** (2026-07-16) como figura de abertura do capítulo de Arquitetura
> (`\label{fig:f23_arquitetura_planos}`), enfatizando os 4 planos + caminho crítico data-centric
> + backends de compatibilidade — complementar à `fig:arquitetura` já existente
> (`arquitetura_geral_v5.png`, mais focada em subsistemas/storage/QoS-fuzzy).
>
> **`F19.png` também foi INCLUÍDA** (2026-07-16) na subseção OP4 (`\label{fig:f19_op4_detalhe}`),
> como detalhe do protocolo de medição (4 fases, 4 pontos de medição, captura DSCP `tos 0x00`) —
> complementa a F18 (visão geral do OP4). **Agora TODAS as F1–F34 estão referenciadas** (nenhuma órfã).
>
> **Pendências para o autor:** (1) gerar/inserir o gráfico da decomposição E1 (T1–T6, ainda um
> `% TODO`); (2) para compilar, instalar o pacote `ulem` (dependência da classe `abntex2`,
> ausente neste ambiente — ex.: `texlive-ulem`); (3) conferir a GPU AMD dos resultados (rótulo
> `rx6600m` vs a `RX 7900 XTX` do ambiente); (4) opcional: consolidar as DUAS figuras de
> arquitetura geral (`F23.png` em planos + `arquitetura_geral_v5.png` em subsistemas) se preferir uma só.

> ## 🔴 BUG CONFIRMADO — `\includegraphics` aponta para o arquivo errado a partir de F19
> **F1–F18.png batem** com as legendas do `.tex` (offset 0). **A partir da figura F19
> (referenciada no `.tex`), o caminho `\includegraphics{img/Fn.png}` aponta um arquivo baixo
> demais**: o conteúdo correto da figura N do `.tex` (N≥19) está em **`F(N+1).png`**.
>
> **Causa:** duas figuras **extras** foram inseridas na numeração dos PNGs, mas os
> `\includegraphics` não foram atualizados: **`F19.png`** (uma 2ª figura de OP4, detalhada) e
> **`F23.png`** (o diagrama de arquitetura GERAL). Como o `.tex` também *pula* o número F22, o
> deslocamento líquido fica em **+1** de F20.png em diante.
>
> **Consequência:** no PDF, as figuras das seções de Metodologia/Arquitetura/Resultados
> renderizam **trocadas** (cada legenda mostra a figura anterior). E a **última** referência do
> `.tex` (F34 = gráfico de resultado E1) **não tem arquivo correspondente** (não existe F35.png).
>
> **Correção sugerida ao autor:** renomear os PNGs para casar com os `\includegraphics`, OU
> corrigir os caminhos no `.tex` (de F19 em diante, usar `F(N+1).png`), e localizar/gerar o
> arquivo do resultado E1 (F34 no `.tex`).

## Mapa definitivo arquivo → conteúdo real → figura do `.tex`
| Arquivo PNG | Conteúdo REAL (verificado) | = figura do `.tex` |
|---|---|---|
| F1 | Entidades DDS/DCPS (Participant/Pub/Sub/Writer/Reader/Topic) | F1 ✓ |
| F2 | Modelo data-centric publish-subscribe | F2 ✓ |
| F3 | Descoberta automática (SPDP/SEDP, matching Topic+Type+QoS) | F3 ✓ |
| F4 | 8 políticas de QoS → tópicos do sistema | F4 ✓ |
| F5 | Inferência generativa (decoder autorregressivo, pipeline completo) | F5 ✓ |
| F6 | Tokenização e geração incremental (TTFT/ITL) | F6 ✓ |
| F7 | Pipeline de implantação local de LLM (GGUF→llama.cpp→GPU→gateway) | F7 ✓ |
| F8 | Padrões de coordenação multiagente (seq/paralelo/hierárquico/estado compartilhado) | F8 ✓ |
| F9 | Arquitetura do MCP + camada de governança | F9 ✓ |
| F10 | Comparação HTTP/REST × gRPC × DDS | F10 ✓ |
| F11 | Mapa do ecossistema de orquestração LLM (por camadas) | F11 ✓ |
| F12 | Quadrantes de posicionamento + lacuna de pesquisa | F12 ✓ |
| F13 | Fluxo metodológico da pesquisa (9 etapas + ciclos) | F13 ✓ |
| F14 | Topologia física do ambiente (Proxmox, VMs, RTX 3080, **RX 7900 XTX**, MacBook) | F14 ✓ |
| F15 | Modelo experimental em dois níveis (transporte × arquitetura) | F15 ✓ |
| F16 | E1: instrumentação/decomposição de latência (T1–T6) | F16 ✓ |
| F17 | OP3: detecção, recuperação e failover | F17 ✓ |
| F18 | OP4: priorização sob carga (visão geral DDS/HTTP/gRPC) | F18 ✓ |
| **F19** | **OP4: priorização sob carga (DETALHADO, timeline + DSCP)** — **EXTRA** | *(não citada)* |
| F20 | OP1/OP2: escalabilidade, utilização e fairness (Jain/Gini) | **F19** ⚠ |
| F21 | E5: streaming token-a-token (TTFT/ITL, DDS/HTTP-SSE/gRPC) | **F20** ⚠ |
| F22 | Matriz de experimentos × hipóteses H0–H6 (título interno: "F21") | **F21** ⚠ |
| **F23** | **Arquitetura GERAL do DDS-LLM-Orchestrator (4 planos, todos subsistemas)** — **EXTRA** | *(não citada / "F22" pulada)* |
| F24 | Espaço global de dados: os 11 tópicos em 4 grupos (chave/produtor/consumidor/QoS) | **F23** ⚠ |
| F25 | Máquina de estados da tarefa (CREATED→PENDING→CLAIMED→RUNNING→COMPLETED + recuperação) | **F24** ⚠ |
| F26 | Sequência completa de uma requisição (12 passos, monitor observa) | **F25** ⚠ |
| F27 | Separação modelo de domínio (Python) × tipos wire DDS + adapters DDS/HTTP/gRPC | **F26** ⚠ |
| F28 | Observabilidade e monitoramento de QoS (eventos CycloneDDS→collector→BD) | **F27** ⚠ |
| F29 | Distribuição de política por snapshot (YAML→PolicyEngine→SecurityPolicy→caches) | **F28** ⚠ |
| F30 | Fluxo de chamada de ferramenta com MCP (governança) | **F29** ⚠ |
| F31 | Organização dos módulos de software (interfaces `I*`, abstração de transporte) | **F30** ⚠ |
| F32 | Mapa de módulos → nós de implantação (portas, GPUs, PostgreSQL, OpenRouter) | **F31** ⚠ |
| F33 | Integração DDS com llama.cpp/llama-server (bridge C++, LLAMA_DDS=ON, IDL) | **F32** ⚠ |
| F34 | Caminhos nativos comparados (DDS/HTTP/gRPC até o llama-server) | **F33** ⚠ |
| *(faltando)* | Resultado E1: decomposição de latência por camada (barras empilhadas) | **F34** ❌ sem arquivo |

## Gráficos de resultados (E1–E5, por GPU)
Existem em duas variantes (`*.png` e `* 2.png`) para RTX 3080 e RX 6600M — **confirmar a
canônica**. Nota: o ambiente da dissertação (F14) usa RTX 3080 e **RX 7900 XTX**, mas os
gráficos são rotulados `rtx3080`/`rx6600m` — verificar se a GPU AMD dos resultados é a
6600M ou a 7900 XTX (possível divergência).
| Prefixo | Conteúdo |
|---|---|
| `E1_latency_boxplot_*`, `E1_latency_comparison_*` | E1 — latência (boxplot e comparação) |
| `E2_failure_detection_*` | E2/OP3 — detecção de falha |
| `E3_priority_comparison_*` | E3/OP4 — priorização |
| `E4_scalability_*` | E4/OP1 — escalabilidade |
| `E5_ttft_comparison_*` | E5 — TTFT |

## Achados arquiteturais úteis à migração (de F23/F24/F27/F31/F32/F33)
Ver `DISSERTACAO.md` — 4 planos, 11 tópicos, subsistemas (policy-engine/mcp-gateway/
context-store/observability), abstração de transporte `ITransport`+adapters, implantação
(a RX 7900 XTX roda agente+gateways) e a fronteira C++ (bridge llama.cpp DDS, que permanece).
