"""CLI do laya-worker (T-900-M2-01).

Uso:
  python -m laya_worker --domain 211 --mock --max-intents 5
  laya-worker --domain 211 --mock --idle-timeout 30

`--mock` usa FakeLayaAdapter (regras, offline). Sem `--mock`, exige
`--checkpoint-sha256` (RealLayaAdapter; R-LAYA BLOCKED levanta causa tipada).
O worker NÃO tem flags para publicar Tasks — a fronteira é estrutural.
"""

from __future__ import annotations

import argparse
import json
import sys


def main(argv: list[str] | None = None) -> int:
    ap = argparse.ArgumentParser(prog="laya-worker")
    ap.add_argument("--domain", type=int, default=211)
    ap.add_argument("--mock", action="store_true", help="FakeLayaAdapter (regras, offline)")
    ap.add_argument("--checkpoint-sha256", default=None,
                    help="hash do checkpoint p/ adapter real (gate R-LAYA)")
    ap.add_argument("--max-intents", type=int, default=None)
    ap.add_argument("--idle-timeout", type=float, default=30.0)
    ap.add_argument("--ttl-s", type=float, default=120.0)
    ap.add_argument("--calibration-path", default=None,
                    help="caminho do calibration_v1.json (pin vira calibration_revision)")
    args = ap.parse_args(argv)

    from .adapter import FakeLayaAdapter, RealLayaAdapter

    if args.mock:
        laya = FakeLayaAdapter()
    else:
        laya = RealLayaAdapter(checkpoint_sha256=args.checkpoint_sha256,
                                calibration_path=args.calibration_path)

    from .space import WorkerSpace, serve

    space = WorkerSpace(args.domain)
    stats = serve(space, laya, max_intents=args.max_intents, idle_timeout_s=args.idle_timeout)
    print(json.dumps({"shadow": stats, "tasks_written": 0}), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
