"""laya-worker — worker de decisão Laya da fase 900 (T-900-M2-01).

Fronteira (ADR-001/D2): SOMENTE classificação. Lê `Workflow.Intent`, publica
`Routing.Decision`. Não faz claim, não chama ferramentas, não materializa
Task, não publica em Tasks/TaskOutput (gate M2-04 observa zero publicações).
"""

__version__ = "0.1.0"
