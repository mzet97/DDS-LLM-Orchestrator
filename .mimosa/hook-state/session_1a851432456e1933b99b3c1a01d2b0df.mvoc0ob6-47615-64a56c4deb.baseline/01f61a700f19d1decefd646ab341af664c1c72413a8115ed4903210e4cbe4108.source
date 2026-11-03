"""CLI do laya-worker (T-900-M2-01; T904-M3).

Uso (T900 local, T904):
  python -m laya_worker --domain 231 \
    --checkpoint-path ~/.cache/tese-laya/checkpoints/laya-t900-specialized-v1 \
    --checkpoint-sha256 358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f

Uso legado:
  python -m laya_worker --domain 211 --mock --max-intents 5
  laya-worker --domain 211 --checkpoint-sha256 891102d3...   (BASE remoto pinado)

`--mock` usa FakeLayaAdapter (regras, offline). Sem `--mock`, sem
`--checkpoint-path`: RealLayaAdapter no ID remoto do BASE (pin obrigatório);
com `--checkpoint-path` + `--checkpoint-sha256`: checkpoint LOCAL explícito
(hash verificado antes do load; sem fallback remoto — T904/REQ-904-001..003).
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
    ap.add_argument("--checkpoint-path", default=None,
                    help="diretório do checkpoint LOCAL completo (T904; exige --checkpoint-sha256)")
    ap.add_argument("--checkpoint-sha256", default=None,
                    help="hash do checkpoint p/ adapter real (gate R-LAYA)")
    ap.add_argument("--model-id", default=None,
                    help="identidade na decisão (default: local:<nome do dir>)")
    ap.add_argument("--max-intents", type=int, default=None)
    ap.add_argument("--idle-timeout", type=float, default=30.0)
    ap.add_argument("--ttl-s", type=float, default=120.0)
    ap.add_argument("--calibration-path", default=None,
                    help="caminho do calibration_v1.json (pin vira calibration_revision)")
    args = ap.parse_args(argv)

    if args.checkpoint_path and not args.checkpoint_sha256:
        ap.error("--checkpoint-sha256 é obrigatório junto de --checkpoint-path")

    from .adapter import FakeLayaAdapter, RealLayaAdapter

    if args.mock:
        laya = FakeLayaAdapter()
    else:
        laya = RealLayaAdapter(checkpoint_path=args.checkpoint_path,
                               checkpoint_sha256=args.checkpoint_sha256,
                               model_id=args.model_id,
                               calibration_path=args.calibration_path)

    from .space import WorkerSpace, serve

    space = WorkerSpace(args.domain)
    stats = serve(space, laya, max_intents=args.max_intents, idle_timeout_s=args.idle_timeout)
    print(json.dumps({"shadow": stats, "tasks_written": 0}), flush=True)
    return 0


if __name__ == "__main__":
    sys.exit(main())
