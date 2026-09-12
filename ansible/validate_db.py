#!/usr/bin/env python3
"""Valida preservação do OperationLog do studio-noded (esquema real).

Esquema persistido: {"owned_services": [...],
"records": {op_id: {"id": ..., "op": ...}}, "order": [op_id, ...]}.
Uso: validate_db.py <atual.json> <backup.json> [--allow-absent]
  --allow-absent: admite primeira instalação (backup inexistente).
Sem a flag, backup ausente é FALHA. JSON inválido é FALHA.
Admitido: apenas acréscimos (novos registros no fim da ordem).
Falha: registro removido/alterado, serviço próprio perdido, ordem
rearranjada.
"""

import json
import sys


def fail(message):
    print(f"validate_db FALHOU: {message}", file=sys.stderr)
    sys.exit(1)


def main():
    args = [a for a in sys.argv[1:] if not a.startswith("--")]
    allow_absent = "--allow-absent" in sys.argv[1:]
    if len(args) != 2:
        fail("uso: validate_db.py <atual.json> <backup.json> [--allow-absent]")
    current_path, backup_path = args
    try:
        with open(current_path) as handle:
            current = json.load(handle)
    except (OSError, json.JSONDecodeError) as err:
        fail(f"atual ilegivel: {err}")
    try:
        with open(backup_path) as handle:
            backup = json.load(handle)
    except FileNotFoundError:
        if allow_absent:
            print("backup ausente admitido (primeira instalacao)")
            return
        fail("backup esperado ausente (passe o caminho exato do backup desta implantacao)")
    except (OSError, json.JSONDecodeError) as err:
        fail(f"backup ilegivel: {err}")

    for key in ("owned_services", "records", "order"):
        if not isinstance(current.get(key), (list, dict)) or not isinstance(backup.get(key), (list, dict)):
            fail(f"esquema inesperado: chave {key!r} ausente ou com tipo errado")

    lost_services = [s for s in backup["owned_services"] if s not in current["owned_services"]]
    if lost_services:
        fail(f"servicos proprios perdidos: {lost_services}")

    for op_id, record in backup["records"].items():
        current_record = current["records"].get(op_id)
        if current_record is None:
            fail(f"registro removido: {op_id}")
        if current_record != record:
            fail(f"registro alterado: {op_id}")

    backup_order = [op for op in backup["order"] if op in backup["records"]]
    positions = []
    for op_id in backup_order:
        try:
            positions.append(current["order"].index(op_id))
        except ValueError:
            fail(f"registro fora da ordem atual: {op_id}")
    if positions != sorted(positions):
        fail("ordem dos registros rearranjada (admitido só acréscimo no fim)")

    print(f"ops_preservadas={len(backup['records'])}")


if __name__ == "__main__":
    main()
