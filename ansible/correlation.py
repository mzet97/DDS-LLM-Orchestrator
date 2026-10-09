#!/usr/bin/env python3
"""Validação de correlação submissão→inferência→saída (contrato do det-responder).

Contrato (crates/det-responder/src/endpoint.rs): cada registro do log
carrega os campos EXATOS ``task_id`` e ``request_id`` — correlação é
igualdade de campo, não busca textual.

A MESMA função decide o positivo e o negativo:
  check(expected_task_id, records)
    -> (ok: bool, reason: str)
Falhas distintas exigidas:
  - EVIDENCE_ABSENT   : nenhum registro casa task_id (ausência de evidência)
  - REQUEST_ID_MISMATCH: registro casa task_id mas request_id diverge
  - (o caso "resposta de OUTRA tarefa" entra como EVIDENCE_ABSENT quando
    apresentada como candidata: a função não recebe lista pré-filtrada —
    quem chama entrega TODOS os registros observados)
"""

import json
import sys


def check(expected_task_id, records):
    """Confere os campos de correlação de `expected_task_id` em `records`.

    `records` é a lista COMPLETA de registros observados (JSON dicts);
    sem filtragem prévia por parte de quem chama.
    """
    if not expected_task_id:
        return False, "EMPTY_TASK_ID"
    matches = [e for e in records if e.get("task_id") == expected_task_id]
    if not matches:
        return False, "EVIDENCE_ABSENT"
    entry = matches[0]
    if entry.get("request_id") != expected_task_id:
        return False, "REQUEST_ID_MISMATCH"
    return True, "OK"


def load_records(path):
    with open(path) as handle:
        return [json.loads(line) for line in handle if line.strip()]


def main():
    if len(sys.argv) != 3:
        print("uso: correlation.py <expected_task_id> <resp.jsonl>", file=sys.stderr)
        return 2
    expected, records_path = sys.argv[1], sys.argv[2]
    records = load_records(records_path)
    ok, reason = check(expected, records)
    print("correlation:%s:%s" % ("OK" if ok else "FAIL", reason))
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
