#!/usr/bin/env python3
"""Autoteste do validate_db.py com fixtures (positivos + negativos).

Executar: python3 test_validate_db.py — saída 0 = tudo certo.
"""

import json
import os
import subprocess
import sys
import tempfile

SCRIPT = os.path.join(os.path.dirname(os.path.dirname(os.path.abspath(__file__))), "validate_db.py")

BASE = {
    "owned_services": ["dds-agent"],
    "records": {
        "op-1": {"id": "op-1", "op": {"SetService": {"service": "dds-agent", "running": True}}},
        "op-2": {"id": "op-2", "op": {"SetService": {"service": "dds-agent", "running": False}}},
    },
    "order": ["op-1", "op-2"],
}


def run(current, backup, extra=(), write_backup=True):
    tmp = tempfile.mkdtemp(prefix="vdb-")
    cur = os.path.join(tmp, "cur.json")
    bak = os.path.join(tmp, "bak.json")
    with open(cur, "w") as handle:
        json.dump(current() if callable(current) else current, handle)
    if write_backup:
        with open(bak, "w") as handle:
            json.dump(backup() if callable(backup) else backup, handle)
    proc = subprocess.run(
        [sys.executable, SCRIPT, cur, bak, *extra],
        capture_output=True,
        text=True,
    )
    return proc


def check(name, proc, must_pass):
    ok = (proc.returncode == 0) == must_pass
    print(("PASS" if ok else "FAIL"), name, "->", (proc.stdout + proc.stderr).strip().splitlines()[-1:])
    if not ok:
        sys.exit(1)


def main():
    import copy

    appended = copy.deepcopy(BASE)
    appended["records"]["op-3"] = {"id": "op-3", "op": {"SetService": {"service": "dds-agent", "running": True}}}
    appended["order"] = ["op-1", "op-2", "op-3"]
    check("acrescimo admitido", run(appended, BASE), True)

    removed = copy.deepcopy(BASE)
    del removed["records"]["op-1"]
    removed["order"] = ["op-2"]
    check("NEGATIVO registro removido", run(removed, BASE), False)

    altered = copy.deepcopy(BASE)
    altered["records"]["op-2"]["op"]["SetService"]["running"] = True
    check("NEGATIVO registro alterado", run(altered, BASE), False)

    lost = copy.deepcopy(BASE)
    lost["owned_services"] = []
    check("NEGATIVO servico perdido", run(lost, BASE), False)

    reordered = copy.deepcopy(BASE)
    reordered["order"] = ["op-2", "op-1"]
    check("NEGATIVO ordem trocada", run(reordered, BASE), False)

    check("NEGATIVO backup ausente sem flag", run(BASE, BASE, write_backup=False), False)
    check("ausencia inicial com flag", run(BASE, BASE, extra=("--allow-absent",), write_backup=False), True)

    tmp = tempfile.mkdtemp(prefix="vdb-")
    bad = os.path.join(tmp, "bad.json")
    with open(bad, "w") as handle:
        handle.write("{json invalido")
    proc = subprocess.run([sys.executable, SCRIPT, bad, bad], capture_output=True, text=True)
    check("NEGATIVO json invalido", proc, False)

    print("validate_db: todos os casos OK")


if __name__ == "__main__":
    main()
