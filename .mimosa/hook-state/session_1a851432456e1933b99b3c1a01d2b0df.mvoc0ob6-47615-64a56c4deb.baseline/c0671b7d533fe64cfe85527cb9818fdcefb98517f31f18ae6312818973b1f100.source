"""Baseline de regras determinístico (T-900-M2-03).

Referência de comparação para o Laya (static/rules do protocolo M6) e motor
do `FakeLayaAdapter`. Palavras-chave PT/EN, sem rede, sem pesos. As
probabilidades são SUAVIZADAS (não 0/1) para exercer o caminho de
distribuições; são HONESTAMENTE não calibradas (baseline de engenharia).

MODEL_ID entra no `RoutingDecision.model_id` com prefixo — rastreabilidade de
que a decisão veio de regras, não do modelo (anti-alegação D10/Art. III).
"""

from __future__ import annotations

from dataclasses import dataclass, field

MODEL_ID = "rules-baseline-v1"

# (padrões PT+EN) -> perfil. Ordem importa: primeiro casamento ganha.
_QOS_RULES: list[tuple[tuple[str, ...], str]] = [
    (("streaming", "stream", "incremental", "interativo", "interative", "ao vivo"), "QI_STREAM_LIKE"),
    (("audit", "auditoria", "security", "seguranc", "critical", "critico", "crítico", "produção", "producao", "production"), "QI_CRITICAL"),
    (("cheap", "barato", "batch", "lote", "custo baixo", "low cost", "economy", "econômico", "economico"), "QI_LOW_COST"),
    (("failover", "redundan", "resilien", "indisponibil", "unavailable"), "QI_FAILOVER"),
]
_DEFAULT_QOS = "QI_BALANCED"

_CAPABILITY_RULES: dict[str, tuple[str, ...]] = {
    "rust_refactor": ("rust", "cargo", "borrow checker", "unsafe"),
    "python_refactor": ("python", "pytest", "pandas", "django"),
    "unit_testing": ("test", "teste", "unittest", "tdd", "cobertura", "coverage"),
    "security_audit": ("security", "seguranc", "vulnerab", "cve", "audit", "auditoria", "injection"),
    "code_review": ("review", "revis", "pull request", "pr ", "diff"),
    "research": ("research", "pesquis", "paper", "artigo", "survey", "benchmark"),
    "summarization": ("summar", "resum", "abstract", "tl;dr", "síntese", "sintese"),
    "translation": ("translat", "traduz", "traduc", "translate"),
}

_PLANNING_HINTS = (" e depois ", " and then ", "depois disso", "em seguida", "then",
                   "após", "after that", "pipeline", "workflow", "etapas", "steps",
                   "primeiro", "first", "por fim", "finally", "depend")

_COMPLEXITY_HINTS = (("arquitetura", "architecture", "distribuíd", "distribuid", "distributed",
                      "refator", "refactor", "migra", "concorr", "concurren", "race"),)


def _matches(text: str, patterns: tuple[str, ...]) -> bool:
    t = text.lower()
    return any(p in t for p in patterns)


@dataclass
class RulesPrediction:
    qos_profile: str
    qos_probabilities: dict[str, float] = field(default_factory=dict)
    needs_planning_probability: float = 0.5
    capability_probabilities: dict[str, float] = field(default_factory=dict)
    complexity_expected: float = 1.0
    urgency_expected: float = 1.0
    priority_probabilities: list[float] = field(default_factory=lambda: [1 / 3, 1 / 3, 1 / 3])


def _smoothed(probs: dict[str, float], alpha: float = 0.04) -> dict[str, float]:
    """Suavização aditiva leve: nenhum perfil com probabilidade 0."""
    n = len(probs)
    return {k: (v + alpha) / (1.0 + alpha * n) for k, v in probs.items()}


def predict(prompt: str) -> RulesPrediction:
    text = prompt.lower()
    qos = _DEFAULT_QOS
    for pats, profile in _QOS_RULES:
        if _matches(text, pats):
            qos = profile
            break
    # distribuição pico no escolhido (não calibrada; baseline)
    raw = {p: (0.80 if p == qos else 0.05) for p in
           ("QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER")}
    qos_probs = _smoothed(raw)

    hits = {cap: 0.88 for cap, pats in _CAPABILITY_RULES.items() if _matches(text, pats)}
    if not hits:
        hits = {"summarization": 0.55}  # fallback conservador p/ pedido genérico
    caps = _smoothed(hits)

    needs = 0.8 if _matches(text, _PLANNING_HINTS) else 0.15

    n_caps = len([1 for c in caps.values() if c > 0.5])
    complexity = min(3.0, 0.5 + 0.6 * n_caps + (0.8 if needs > 0.5 else 0.0))

    urgent = _matches(text, ("urgente", "urgent", "asap", "agora", "hoje", "imediat",
                             "produção", "producao", "production", "incidente", "incident"))
    urgency = 1.9 if urgent else (1.2 if needs > 0.5 else 0.7)

    # prioridade a partir de urgency (esperança em 0..2 mapeada em LOW/NORMAL/HIGH)
    if urgency >= 1.75:
        prio = [0.05, 0.20, 0.75]
    elif urgency >= 1.0:
        prio = [0.15, 0.65, 0.20]
    else:
        prio = [0.70, 0.22, 0.08]

    return RulesPrediction(
        qos_profile=qos,
        qos_probabilities=qos_probs,
        needs_planning_probability=needs,
        capability_probabilities=caps,
        complexity_expected=complexity,
        urgency_expected=urgency,
        priority_probabilities=prio,
    )
