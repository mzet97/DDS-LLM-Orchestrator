# Plano de ablacoes T903 (congelado pre-treino, 2026-10-20)
Controles: B0 = T900 (35813054...) inferencia no dev novo (QoS 0.62/0.545, stream_rec 0.0). B1 = Arm B T902 (5d7dd652...) somente avaliacao no dev (pesos locais disponiveis) — sem re-treino.
H1 (diversidade lexical): C1 = fine-tune T900 no train limpo (RLCD). H2 (contraste+replay): C2 = C1 + focal dirigido. H3 (estabilidade): C3 = C1 SoftCE. C4 (base direta) somente se orcamento permitir.
Fator isolado por braco: dados(perda). Seeds: piloto 1 seed (902); finalista ate 3 se variancia duvidosa e orcamento permitir.
Metricas primarias (dev agrupado): STREAM recall sujeito a precision>=0.5, QoS Macro-F1, nao-regressao multitarefa. Margens KEEP: stream>=0.75 E QoS F1>=0.85 E urg F1>=0.73 E planning>=0.98 E complex F1>=0.94 E caps>=0.97; golden antigo sem regressao alem de 0.01 QoS F1.
Custo estimado: 3 treinos ~1300-1600 updates cada ≈ 75-95 min/T4 cada + avaliacoes; total ~5-6h (~6 units), saldo ~374.
