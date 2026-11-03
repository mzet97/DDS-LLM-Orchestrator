"""Perguntas versionadas do Laya (T-900-M2-02).

O conjunto de perguntas, rótulos, idiomas e thresholds é VERSIONADO
(contracts.md: "Perguntas, rótulos, idiomas, ordem de opções, thresholds,
política e checkpoint são versionados"). Um resultado sem revisão é inválido.

Os rótulos dos `choice`/`score` seguem EXATAMENTE os literais do contrato
`WorkflowPlanning.idl` (emenda de prefixos M1-01) — o parser recusa rótulo
fora do conjunto.
"""

from typing import Any

QUESTIONS_REVISION = 1

QOS_PROFILES = ["QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"]

CAPABILITY_CATALOG_REVISION = 1
CAPABILITY_CATALOG = [
    "rust_refactor",
    "python_refactor",
    "unit_testing",
    "security_audit",
    "code_review",
    "research",
    "summarization",
    "translation",
]

COMPLEXITY_LEVELS = ["simple", "moderate", "complex", "very_complex"]
URGENCY_LEVELS = ["low", "normal", "high"]

# Perguntas no formato `laya` (choice/noul/score). A ordem das opções é parte
# da revisão (contracts.md) — não reordenar sem nova revisão.
QUESTIONS_V1: dict[str, Any] = {
    "qos_profile": {
        "type": "choice",
        "instructions": "Which QoS objective best fits this workload?",
        "criteria": {
            "QI_BALANCED": "balanced latency and resource use",
            "QI_CRITICAL": "deadline and reliability-sensitive work",
            "QI_STREAM_LIKE": "interactive incremental streaming",
            "QI_LOW_COST": "resource-efficient batch work",
            "QI_FAILOVER": "resilience when execution resources are unavailable",
        },
    },
    "needs_planning": {
        "type": "noul",
        "instructions": "Does the request require multiple dependent tasks?",
    },
    "capabilities": {
        "type": "noul",
        "instructions": "Which capabilities does this request require? One independent question per capability.",
        "items": CAPABILITY_CATALOG,
    },
    "complexity": {
        "type": "score",
        "instructions": "How complex is the task?",
        "criteria": COMPLEXITY_LEVELS,
    },
    "urgency": {
        "type": "score",
        "instructions": "How urgent is the task?",
        "criteria": URGENCY_LEVELS,
    },
}


def questions_for_revision(revision: int) -> dict[str, Any]:
    if revision != QUESTIONS_REVISION:
        raise ValueError(f"questions_revision {revision} não suportada (ativa: {QUESTIONS_REVISION})")
    return QUESTIONS_V1
