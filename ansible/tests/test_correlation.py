"""Testes do verificador de correlação (positivos e negativos reais)."""
import sys
import os

sys.path.insert(0, os.path.join(os.path.dirname(__file__), ".."))
from correlation import check  # noqa: E402


def _rec(task_id, request_id=None):
    return {"task_id": task_id, "request_id": request_id or task_id, "agent_id": "x"}


T1 = "11111111-1111-1111-1111-111111111111"
T2 = "22222222-2222-2222-2222-222222222222"


def test_positivo_tarefa1():
    ok, reason = check(T1, [_rec(T2), _rec(T1)])
    assert ok and reason == "OK"


def test_positivo_tarefa2():
    ok, reason = check(T2, [_rec(T2), _rec(T1)])
    assert ok and reason == "OK"


def test_negativo_tarefa2_apresentada_como_tarefa1():
    # A evidência da tarefa 2 NÃO satisfaz a tarefa 1 — mesma função,
    # lista completa entregue (sem filtragem prévia).
    ok, reason = check(T1, [_rec(T2), _rec(T2)])
    assert not ok and reason == "EVIDENCE_ABSENT"


def test_negativo_ausencia_de_evidencia():
    ok, reason = check(T1, [])
    assert not ok and reason == "EVIDENCE_ABSENT"


def test_negativo_request_id_divergente():
    # Registro com task_id certo mas request_id de OUTRA tarefa.
    ok, reason = check(T1, [{"task_id": T1, "request_id": T2}])
    assert not ok and reason == "REQUEST_ID_MISMATCH"


def test_negativo_task_id_vazio():
    ok, reason = check("", [_rec(T1)])
    assert not ok and reason == "EMPTY_TASK_ID"
