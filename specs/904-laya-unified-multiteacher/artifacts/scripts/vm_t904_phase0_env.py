#!/usr/bin/env python3
"""VM-side T904 fase 0: ambiente + dependências pinadas. Marcadores CHUNK_*."""
import subprocess
import sys


def sh(c):
    return subprocess.run(c, shell=True, capture_output=True, text=True)


print("CHUNK_ENV_BEGIN", flush=True)
print(sh("nvidia-smi --query-gpu=name,memory.total,driver_version --format=csv,noheader").stdout.strip())
print("python:", sys.version.split()[0])
r = sh("pip install -q laya==0.4.10 2>&1 | tail -1")
print("pip laya:", r.stdout.strip() or r.stderr.strip()[-200:])
import torch  # noqa: E402
import laya  # noqa: E402

print("torch", torch.__version__, "cuda:", torch.cuda.is_available())
print("laya import ok")
print(sh("df -h /content | tail -1").stdout.strip())
print(sh("laya-train --help 2>&1 | head -3").stdout.strip())
print("CHUNK_ENV_END", flush=True)
