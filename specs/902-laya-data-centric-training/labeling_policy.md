# Politica de rotulagem T902 rev.1 (DATA FIRST)

Perfil logico de aplicacao (Laya) != QoS DDS nativo (Reliability/Durability/Deadline - ADR-002). Rotulo vem do texto observavel + contexto disponivel ao Laya.

## QoS (precedencia: entrega > consequencia > economia)
- QI_STREAM_LIKE: entrega incremental/interativa pedida ou implicita (acompanhar em partes, ao vivo). Palavra "streaming" como topico != entrega streaming (negativo).
- QI_CRITICAL: prazo curto explicito OU producao/consequencia severa explicita.
- QI_FAILOVER: pedido de resiliencia/retentativa/continuidade diante de falha.
- QI_LOW_COST: lote sem urgencia, economia/eficiencia como objetivo explicito.
- QI_BALANCED: demais casos (entrega completa, ritmo normal).
Ambiguidade sem sinal observavel: ABSTAIN/REVIEW na curadoria (nunca classe nova).

## Urgencia (prazo observavel manda; palavra isolada nao)
- high: hoje/horas, producao parada. normal: esta semana/amanha. low: sem pressa/semanas ou sem deadline.
- Complexidade independente de urgencia (fatorial obrigatorio).

## Consistencia
Texto gerado/escrito ANTES do rotulo; rotulo derivado dos fatores observaveis registrados em evidence_span. Dupla checagem automatica de regras (deadline/modo) + amostra estratificada auditada; discordancia vira REVIEW e sai da supervisao.
