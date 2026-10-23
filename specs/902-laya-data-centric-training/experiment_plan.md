# Plano de experimentos pre-registrado
Hipoteses: H1 sinal observavel melhora STREAM; H2 SoftCE estabiliza vs RLCD; H3 focal direcionado sem vazar grupo melhora STREAM/urg-low.
Bracos (mesmos dados limpos, seed 902, 4 epocas): A=T900+RLCD replay; B=T900+focal (duplicacao intra-treino STREAM/urg-low); C=T900+SoftCE. Finalista: melhor no dev agrupado (QoS F1, STREAM rec, urg F1, nao-regressao).
Gates M6: QoS acc>=0.90 F1>=0.88 STREAM>=0.75 urgF1>=0.73; planning>=0.98 complexF1>=0.94 caps>=0.97; nao-regressao com IC (margem 0.01 QoS, 0.02 demais). Sealed aberto 1x apos freeze.
Custo estimado: 3 bracos ~4.5h + avaliacoes ~1h T4.
