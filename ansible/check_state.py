#!/usr/bin/env python3
"""Estado não vazio PELO ESQUEMA: as operações esperadas existem em
records E em order do OperationLog ({"owned_services": [...],
"records": {op_id: {...}}, "order": [op_id, ...]}).
Uso: check_state.py <db.json> <op_id> [<op_id>...]
"""
import json
import sys


def main():
    if len(sys.argv) < 3:
        print("uso: check_state.py <db.json> <op_id>...", file=sys.stderr)
        return 2
    db = json.load(open(sys.argv[1]))
    records = db.get("records", {})
    order = db.get("order", [])
    missing = [op for op in sys.argv[2:] if op not in records or op not in order]
    if missing:
        print("check_state FALHOU: operacoes ausentes do esquema: %s" % missing)
        return 1
    print("check_state OK: %d operacoes presentes em records/order; owned_services=%s"
          % (len(sys.argv) - 2, ",".join(db.get("owned_services", []))))
    return 0


if __name__ == "__main__":
    sys.exit(main())
