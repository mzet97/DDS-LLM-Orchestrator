# Plano de experimentos T909 (pré-registrado antes do treino)

Braços (máx 3, seed 909, ≤6 épocas, soft-ce, early-stop min-F1 dev 2 épocas):
- A: init T908-a (bced0c97) + treino = pool T908 limpo + corpus novo v6 (hipótese: rubrica+rótulos resolvem urg/cx/critical)
- B: init C2 (624de57e) + MESMO treino/orçamento (controle de init; protege STREAM)
- C: NÃO EXECUTADO por padrão (SDK laya 0.4.10 só suporta soft-ce/rlcd; variante ordinal exigiria loss custom fora do SDK)

Seleção (M8): pior competência individual no dev (min-F1); desempate média harmônica das macros.
Calibração (M9): temperatura por pergunta no CAL; fallback conservador se n insuficiente.
Avaliação final (M10): sealed aberto 1× ; B0/N2/C2/T908-a/T909 juntos; IC95 bootstrap por família.

Orçamento local (sem Colab): geração .61 ~3-5h; treino RX 2 braços × ~1-2h (VRAM 24GB, mb16 possível);
evals CPU ~15min/modelo/conjunto. Energia: medir watts reais (rocm-smi/nvidia-smi) e duração; kWh estimado sem inventar preço.

## Regras de parada e honestidade
- >0.90 é estrito (>0.900000); sem arredondar.
- Nenhuma iteração com feedback do sealed; re-open proibido.
- Se min-F1 dev não superar T908-a após A+B: manter T908-a experimental; sealed avaliado 1× mesmo assim (evidência negativa documentada).
- Divergência CPU/CUDA/ROCm inexplicada em decisões bloqueia promoção (não o relatório).
