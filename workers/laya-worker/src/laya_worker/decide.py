"""Intenção → decisão tipada (T-900-M2-02).

Regras do contrato (contracts.md §Routing.Decision):
- OK exige distribuições completas/consistentes; prioridade esperada em [0,2];
  label de perfil via argmax (empate: LOW→NORMAL→HIGH);
- erro usa reason != RC_NONE; campos de previsão podem ser vazios/zero;
- valid_until não excede deadline da intenção;
- calibration_revision vazia só é aceita como observação SHADOW;
- payload_hash exclui o próprio campo (preimage documental).

O resultado é DADO NÃO CONFIÁVEL até validação Rust (D10) — o worker nunca
decide permissões/ferramentas.
"""

from __future__ import annotations

import math
import time
from typing import Any

from . import adapter as adapter_mod
from .digest import digest_typed
from .questions import CAPABILITY_CATALOG, QUESTIONS_REVISION, QOS_PROFILES

DEFAULT_TTL_S = 120.0
PRODUCER_ID_DEFAULT = "laya-worker-1"


def _now_ns() -> int:
    return time.time_ns()


def _error_decision(intent: Any, reason_ordinal: int, created_ns: int, producer: str) -> dict:
    """Decisão de erro: status DS_ERROR(1), previsões zeradas/vazias."""
    return {
        "request_id": intent.request_id,
        "decision_id": f"dec-{intent.request_id}",
        "workflow_id": intent.workflow_id,
        "schema_version": 1,
        "intent_revision": intent.intent_revision,
        "producer_id": producer,
        "model_id": "unavailable",
        "checkpoint_revision": "",
        "questions_revision": intent.questions_revision,
        "policy_revision": intent.policy_revision,
        "capability_catalog_revision": intent.capability_catalog_revision,
        "qos_profile": 0,
        "qos_probabilities": [],
        "capability_probabilities": [],
        "requires_planning_probability": 0.0,
        "priority_score": 0.0,
        "priority_probabilities": [],
        "confidence": 0.0,
        "answer_confidence": 0.0,
        "calibration_revision": "",
        "created_at_ns": created_ns,
        "valid_until_ns": min(created_ns + int(DEFAULT_TTL_S * 1e9), intent.deadline_ns),
        "decision_status": 1,  # DS_ERROR
        "reason_code": reason_ordinal,
        "evidence_digest": "",
        "payload_hash": "",  # preenchido abaixo
    }


def _finish(decision: dict) -> dict:
    """Computa payload_hash (preimage EXCLUI payload_hash) e congela."""
    typed = _to_typed(decision)
    decision["payload_hash"] = digest_typed(typed, skip_field="payload_hash")
    return decision


def _to_typed(d: dict) -> dict:
    S = lambda v: {"t": "string", "v": v}  # noqa: E731
    I = lambda t, v: {"t": t, "v": v}  # noqa: E731
    E = lambda e, v: {"t": "enum", "v": v, "enum": e}  # noqa: E731

    def seq(elem, items):
        return {"t": "seq", "v": list(items), "elem": elem}

    return {
        "t": "struct",
        "struct": "RoutingDecision",
        "v": {
            "request_id": S(d["request_id"]),
            "decision_id": S(d["decision_id"]),
            "workflow_id": S(d["workflow_id"]),
            "schema_version": I("u16", d["schema_version"]),
            "intent_revision": I("u32", d["intent_revision"]),
            "producer_id": S(d["producer_id"]),
            "model_id": S(d["model_id"]),
            "checkpoint_revision": S(d["checkpoint_revision"]),
            "questions_revision": I("u32", d["questions_revision"]),
            "policy_revision": I("u32", d["policy_revision"]),
            "capability_catalog_revision": I("u32", d["capability_catalog_revision"]),
            "qos_profile": E("QosIntent", d["qos_profile_label"]) if d.get("decision_status") == 0 else E("QosIntent", "QI_BALANCED"),
            "qos_probabilities": seq(
                {"t": "struct", "struct": "ProfileProbability"},
                [
                    {"t": "struct", "struct": "ProfileProbability",
                     "v": {"profile": E("QosIntent", p), "probability": {"t": "f64", "v": prob}}}
                    for p, prob in d["qos_probabilities"].items()
                ],
            )
            if isinstance(d.get("qos_probabilities"), dict)
            else seq({"t": "struct", "struct": "ProfileProbability"}, []),
            "capability_probabilities": seq(
                {"t": "struct", "struct": "CapabilityProbability"},
                [
                    {"t": "struct", "struct": "CapabilityProbability",
                     "v": {"capability_id": S(c), "probability": {"t": "f64", "v": p},
                           "answer_confidence": {"t": "f64", "v": a}}}
                    for c, (p, a) in d["capability_probabilities"].items()
                ],
            )
            if isinstance(d.get("capability_probabilities"), dict)
            else seq({"t": "struct", "struct": "CapabilityProbability"}, []),
            "requires_planning_probability": {"t": "f64", "v": d["requires_planning_probability"]},
            "priority_score": {"t": "f64", "v": d["priority_score"]},
            "priority_probabilities": seq({"t": "f64"}, [
                {"t": "f64", "v": x} for x in d["priority_probabilities"]
            ])
            if d.get("priority_probabilities")
            else seq({"t": "f64"}, []),
            "confidence": {"t": "f64", "v": d["confidence"]},
            "answer_confidence": {"t": "f64", "v": d["answer_confidence"]},
            "calibration_revision": S(d["calibration_revision"]),
            "created_at_ns": I("u64", d["created_at_ns"]),
            "valid_until_ns": I("u64", d["valid_until_ns"]),
            "decision_status": E("DecisionStatus", "DS_OK" if d["decision_status"] == 0 else "DS_ERROR"),
            "reason_code": E("ReasonCode", _RC_BY_ORD[d["reason_code"]]),
            "evidence_digest": S(d["evidence_digest"]),
            "payload_hash": S(d["payload_hash"]),
        },
    }


_RC_LITERALS = [
    "RC_NONE", "RC_INVALID_INPUT", "RC_MODEL_UNAVAILABLE", "RC_TIMEOUT",
    "RC_SCHEMA_MISMATCH", "RC_POLICY_DENIED", "RC_LOW_CONFIDENCE", "RC_EXPIRED",
    "RC_CAPABILITY_UNAVAILABLE", "RC_OVERLOADED", "RC_CONFLICT", "RC_INVALID_PLAN",
    "RC_OUTPUT_INVALID", "RC_IN_DOUBT", "RC_CANCELLED",
]
_RC_BY_ORD = dict(enumerate(_RC_LITERALS))
_RC_ORD = {lit: i for i, lit in enumerate(_RC_LITERALS)}


def decide(intent: Any, laya: adapter_mod.LayaAdapter, *,
           producer_id: str = PRODUCER_ID_DEFAULT, ttl_s: float = DEFAULT_TTL_S,
           now_ns: int | None = None) -> dict:
    """Classifica UMA intenção → decisão (dict de campos do wire, enums como ordinal).

    Nunca levanta para falhas do modelo: viram decisão DS_ERROR com reason
    tipado (MODEL_UNAVAILABLE/TIMEOUT/INVALID_INPUT). Levanta apenas para
    entrada inválida do PRÓPRIO worker (bug, não dado).
    """
    created = now_ns if now_ns is not None else _now_ns()

    # Revisões da intenção devem casar com as perguntas ativas (contracts:
    # versões fixadas por intenção; sem seleção de revisão "mais recente").
    if intent.questions_revision != QUESTIONS_REVISION:
        d = _error_decision(intent, _RC_ORD["RC_SCHEMA_MISMATCH"], created, producer_id)
        return _finish(d)

    try:
        answer = laya.decide(intent.prompt, intent.deadline_ns)
    except adapter_mod.LayaTimeoutError:
        return _finish(_error_decision(intent, _RC_ORD["RC_TIMEOUT"], created, producer_id))
    except adapter_mod.LayaUnavailableError:
        return _finish(_error_decision(intent, _RC_ORD["RC_MODEL_UNAVAILABLE"], created, producer_id))
    except Exception:
        return _finish(_error_decision(intent, _RC_ORD["RC_INVALID_INPUT"], created, producer_id))

    if not adapter_mod.answer_is_self_consistent(answer):
        return _finish(_error_decision(intent, _RC_ORD["RC_OUTPUT_INVALID"], created, producer_id))

    if intent.questions_revision != QUESTIONS_REVISION:  # pragma: no cover
        return _finish(_error_decision(intent, _RC_ORD["RC_SCHEMA_MISMATCH"], created, producer_id))

    # Catálogo: probabilidades só de capacidades conhecidas (D10: sem rótulo inventado).
    known = {c: (answer.capability_probabilities.get(c, 0.02),
                 answer.capability_answer_confidences.get(c, 0.0))
             for c in CAPABILITY_CATALOG}

    priority_score = answer.urgency_expected  # esperança em [0,2]
    if not (0.0 <= priority_score <= 2.0):
        priority_score = min(2.0, max(0.0, priority_score))

    decision = {
        "request_id": intent.request_id,
        "decision_id": f"dec-{intent.request_id}",
        "workflow_id": intent.workflow_id,
        "schema_version": 1,
        "intent_revision": intent.intent_revision,
        "producer_id": producer_id,
        "model_id": answer.model_id,
        "checkpoint_revision": answer.checkpoint_revision,
        "questions_revision": intent.questions_revision,
        "policy_revision": intent.policy_revision,
        "capability_catalog_revision": intent.capability_catalog_revision,
        "qos_profile_label": answer.qos_profile,
        "qos_probabilities": {p: answer.qos_probabilities.get(p, 0.0) for p in QOS_PROFILES},
        "capability_probabilities": known,
        "requires_planning_probability": answer.needs_planning_probability,
        "priority_score": priority_score,
        "priority_probabilities": list(answer.priority_probabilities),
        "confidence": answer.confidence,
        "answer_confidence": answer.answer_confidence,
        "calibration_revision": "",  # shadow: sem calibração válida (contracts)
        "created_at_ns": created,
        "valid_until_ns": min(created + int(ttl_s * 1e9), intent.deadline_ns),
        "decision_status": 0,  # DS_OK
        "reason_code": _RC_ORD["RC_NONE"],
        "evidence_digest": "",
        "payload_hash": "",
    }
    if decision["qos_profile_label"] not in QOS_PROFILES:
        return _finish(_error_decision(intent, _RC_ORD["RC_OUTPUT_INVALID"], created, producer_id))
    return _finish(decision)


def decision_to_wire(d: dict, mod) -> Any:
    """Materializa a decisão no tipo gerado (`dds_llm_workflow_v1`), enums como ordinal."""
    qos_ord = QOS_PROFILES.index(d["qos_profile_label"]) if d.get("qos_profile_label") else 0
    return mod.RoutingDecision(
        request_id=d["request_id"],
        decision_id=d["decision_id"],
        workflow_id=d["workflow_id"],
        schema_version=d["schema_version"],
        intent_revision=d["intent_revision"],
        producer_id=d["producer_id"],
        model_id=d["model_id"],
        checkpoint_revision=d["checkpoint_revision"],
        questions_revision=d["questions_revision"],
        policy_revision=d["policy_revision"],
        capability_catalog_revision=d["capability_catalog_revision"],
        qos_profile=qos_ord,
        qos_probabilities=[
            mod.ProfileProbability(QOS_PROFILES.index(p), prob)
            for p, prob in d["qos_probabilities"].items()
        ] if d.get("qos_probabilities") else [],
        capability_probabilities=[
            mod.CapabilityProbability(c, pair[0], pair[1])
            for c, pair in d["capability_probabilities"].items()
        ] if d.get("capability_probabilities") else [],
        requires_planning_probability=d["requires_planning_probability"],
        priority_score=d["priority_score"],
        priority_probabilities=list(d.get("priority_probabilities") or []),
        confidence=d["confidence"],
        answer_confidence=d["answer_confidence"],
        calibration_revision=d["calibration_revision"],
        created_at_ns=d["created_at_ns"],
        valid_until_ns=d["valid_until_ns"],
        decision_status=d["decision_status"],
        reason_code=d["reason_code"],
        evidence_digest=d["evidence_digest"],
        payload_hash=d["payload_hash"],
    )
