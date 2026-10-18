#!/usr/bin/env python3
"""Driver do CHUNK na VM Colab (T-900-A2).

Espera: llama-server já no ar em 127.0.0.1:8901 (bash serve.sh &) e setup
feito. Gera --n exemplos com --seed do chunk em <out>.jsonl (resumível) e ao
fim roda verify.py no próprio arquivo.

Uso: python3 run_gen.py --n 3000 --seed 101 --out chunk-1.jsonl
"""
import argparse
import subprocess
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parent))
import gen_dataset as g  # noqa: E402


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--n", type=int, required=True)
    ap.add_argument("--seed", type=int, required=True)
    ap.add_argument("--out", required=True)
    args = ap.parse_args()

    out = Path(args.out)
    if out.exists() and out.stat().st_size > 0:
        print(f"[run_gen] {out} já existe com {len(out.read_text().splitlines())} linhas — MERGE, não regenerar", flush=True)

    # gen_dataset.main espera argv estilo CLI
    sys.argv = ["gen_dataset.py", str(out), "--n", str(args.n), "--seed", str(args.seed), "--resume"]
    rc = g.main()

    # validação automática do chunk
    v = subprocess.run([sys.executable, str(Path(__file__).parent / "verify.py"), str(out)])
    return rc if v.returncode == 0 else 1


if __name__ == "__main__":
    sys.exit(main())
