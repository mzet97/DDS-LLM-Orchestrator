#!/usr/bin/env python3
"""VM-side T905 fase 1: monta o checkpoint VIGENTE das partes, confere SHA.
Env: nada (paths fixos /content/laya_t905)."""
import glob
import hashlib
import os

ROOT = "/content/laya_t905"
WANT = "bda679e71295593904370eba5d9130609aea985f0b0d183d2527fcd398ca039a"


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for c in iter(lambda: f.read(1 << 20), b""):
            h.update(c)
    return h.hexdigest()


d = f"{ROOT}/teachers/vigente"
w = f"{d}/model.safetensors"
if not (os.path.exists(w) and sha256(w) == WANT):
    parts = sorted(glob.glob(f"{ROOT}/upload/vigente.parts/*/p*") + glob.glob(f"{ROOT}/upload/vigente.parts/p*"))
    assert parts, f"sem partes em {ROOT}/upload/vigente.parts/"
    print(f"montando {len(parts)} partes…", flush=True)
    os.makedirs(d, exist_ok=True)
    with open(w, "wb") as out:
        for p in parts:
            with open(p, "rb") as f:
                while True:
                    b = f.read(1 << 22)
                    if not b:
                        break
                    out.write(b)
    got = sha256(w)
    assert got == WANT, f"SHA divergente: {got} != {WANT}"
comp = f"{ROOT}/teachers/_complement"
for rel in ("rl_agent_config.json", "encoder/config.json",
            "tokenizer/tokenizer_config.json", "tokenizer/tokenizer.json"):
    dst = f"{d}/{rel}"
    if not os.path.exists(dst):
        os.makedirs(os.path.dirname(dst), exist_ok=True)
        assert os.system(f"cp '{comp}/{rel.replace('/', '_')}' '{dst}'") == 0
print("PHASE1_BASE_DONE", sha256(w)[:12], flush=True)
