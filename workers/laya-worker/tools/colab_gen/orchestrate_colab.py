#!/usr/bin/env python3
"""Orquestrador LOCAL da geração no Colab (T-900-A2).

Resolve os dois problemas da sessão anterior:
1. KEEP-ALIVE: pinga `colab status` a cada 10 min (sessão não morre por
   inatividade enquanto este script roda).
2. PERSISTÊNCIA: baixa o chunk parcial a cada 15 min p/ o repositório
   (colab download) — se a VM morrer, no máximo 15 min de trabalho se perdem.

Uso: orchestrate_colab.py <chunks_dir_local> [--watch]
Pré: sessão Colab 't900-gen' já criada com GPU L4 e setup.sh executado.
"""
import argparse
import subprocess
import sys
import time
from pathlib import Path

COLAB = str(Path.home() / ".local/bin/colab")
SESSION = "t900-gen"


def sh(*args, timeout=120):
    return subprocess.run([COLAB, *args], capture_output=True, text=True, timeout=timeout)


def cell(code: str, timeout=120):
    """exec numa célula remota."""
    import tempfile
    with tempfile.NamedTemporaryFile(suffix=".py", delete=False, mode="w") as f:
        f.write(code)
        path = f.name
    try:
        r = subprocess.run([COLAB, "exec", "-s", SESSION, "-f", path,
                            "--timeout", str(timeout)], capture_output=True, text=True,
                           timeout=timeout + 30)
        return r.stdout
    finally:
        Path(path).unlink(missing_ok=True)


def count_local(d: Path) -> int:
    total = 0
    for f in d.glob("chunk-*.jsonl"):
        total += sum(1 for l in f.read_text().splitlines() if l.strip())
    return total


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("outdir")
    ap.add_argument("--every-min", type=int, default=15)
    args = ap.parse_args()
    out = Path(args.outdir)
    out.mkdir(parents=True, exist_ok=True)

    print(f"[orc] keep-alive + download a cada {args.every_min} min; Ctrl-C p/ parar", flush=True)
    tick = 0
    while True:
        try:
            # 1) keep-alive (status conta como atividade do kernel)
            st = sh("status", timeout=60)
            alive = SESSION in (st.stdout + st.stderr) and "not found" not in (st.stdout + st.stderr)
            # 2) progresso remoto
            prog = cell(
                "import os\n"
                "n = sum(1 for l in open('/content/colab_gen/chunk-1.jsonl') "
                "if l.strip()) if os.path.exists('/content/colab_gen/chunk-1.jsonl') else 0\n"
                "done = os.path.exists('/tmp/gen1.log')\n"
                "print(f'PROG {n} {done}')",
                timeout=90,
            )
            marker = [l for l in prog.splitlines() if l.startswith("PROG ")]
            if marker:
                _, remote_n, remote_done = marker[-1].split(maxsplit=2)
            else:
                remote_n, remote_done = "?", "?"
            # 3) download do parcial (nome único por tick; merge local por dedup)
            if remote_n not in ("?", "0"):
                d = sh("download", "-s", SESSION, "/content/colab_gen/chunk-1.jsonl",
                       str(out / f"partial-{tick:03d}.jsonl"), timeout=180)
                local_n = count_local(out)
                print(f"[orc t{tick}] sessão={'viva' if alive else 'MORTA'} "
                      f"remoto={remote_n} (done={remote_done}) local≈{local_n}", flush=True)
            else:
                print(f"[orc t{tick}] sessão={'viva' if alive else 'MORTA'} remoto={remote_n}",
                      flush=True)
            if remote_done == "True":
                print("[orc] GERAÇÃO CONCLUÍDA — download final", flush=True)
                sh("download", "-s", SESSION, "/content/colab_gen/chunk-1.jsonl",
                   str(out / "chunk-1-FINAL.jsonl"), timeout=300)
                return 0
            if not alive:
                print("[orc] SESSÃO MORTA — encerrando (relançar com --resume)", flush=True)
                return 1
        except Exception as e:
            print(f"[orc t{tick}] erro: {e}", flush=True)
        tick += 1
        time.sleep(args.every_min * 60)


if __name__ == "__main__":
    sys.exit(main())
