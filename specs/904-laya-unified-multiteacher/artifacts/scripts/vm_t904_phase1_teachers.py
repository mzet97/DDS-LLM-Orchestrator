#!/usr/bin/env python3
"""VM-side T904 fase 1: monta professores das partes, confere SHA, carrega e roda
inferência (dev + distill) gravando previsões por qid. Marcadores CHUNK_* no stdout.
Roda via: colab exec -s laya904 -f vm_t904_phase1_teachers.py --timeout 14400
"""
import glob
import hashlib
import json
import os
import time

ROOT = "/content/laya_t904"
TEACHERS = {
    "t900": "358130541619651210825cec965e2141a2650e8d20ec0d3e714cd3f98123a76f",
    "t902b": "5d7dd6522d94347f47566499e09cbd2ad90b4fccbeb26a91c6885fa8c0eda4c0",
    "t903c": "7de213c7b24fca603f1f58e04ad4f8029fef396d5cf0e24e3c0067bb996cd05d",
}


def sh(cmd):
    import subprocess
    return subprocess.run(cmd, shell=True, capture_output=True, text=True)


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for c in iter(lambda: f.read(1 << 20), b""):
            h.update(c)
    return h.hexdigest()


def log(*a):
    print(*a, flush=True)


print("CHUNK_ENV_BEGIN", flush=True)
import torch  # noqa: E402
log("gpu:", torch.cuda.get_device_name(0) if torch.cuda.is_available() else "NONE")
import laya  # noqa: E402
log("laya ok; cuda:", torch.cuda.is_available())
log("disk:", sh("df -h /content | tail -1").stdout.strip())
print("CHUNK_ENV_END", flush=True)

# ---- montar professores ----
for tid, want in TEACHERS.items():
    d = f"{ROOT}/teachers/{tid}"
    w = f"{d}/model.safetensors"
    if os.path.exists(w) and sha256(w) == want:
        log(f"[{tid}] já montado, SHA ok")
        continue
    parts = sorted(glob.glob(f"{ROOT}/upload/{tid}.parts/model.safetensors.parts/p*")
                   + glob.glob(f"{ROOT}/upload/{tid}.parts/p*"))
    assert parts, f"[{tid}] sem partes em {ROOT}/upload/{tid}.parts/"
    log(f"[{tid}] montando {len(parts)} partes…")
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
    assert got == want, f"[{tid}] SHA divergente: {got} != {want}"
    log(f"[{tid}] SHA OK {got[:12]}…")
# complemento compartilhado (rl_agent_config/encoder/tokenizer) já enviado p/ teachers/_complement/
comp = f"{ROOT}/teachers/_complement"
for tid in TEACHERS:
    d = f"{ROOT}/teachers/{tid}"
    for rel in ("rl_agent_config.json", "encoder/config.json",
                "tokenizer/tokenizer_config.json", "tokenizer/tokenizer.json"):
        dst = f"{d}/{rel}"
        if not os.path.exists(dst):
            os.makedirs(os.path.dirname(dst), exist_ok=True)
            shutil_ok = os.system(f"cp '{comp}/{rel.replace('/', '_')}' '{dst}'")
            assert shutil_ok == 0, f"complemento faltando: {rel}"

# ---- inferência dos professores ----
os.makedirs(f"{ROOT}/teachers_preds", exist_ok=True)
os.environ.setdefault("HF_HUB_OFFLINE", "1")


def predict_split(agent, tid, split_path, out_path):
    if os.path.exists(out_path):
        log(f"[{tid}] {os.path.basename(out_path)} já existe — pulando")
        return
    import sys
    sys.path.insert(0, "/content/laya_t904/tools")
    from questions import CAPABILITY_CATALOG, QUESTIONS_V1  # byte-exato do worker
    Q = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
    for c in CAPABILITY_CATALOG:
        Q[f"cap__{c}"] = {"type": "noul",
                          "instructions": f"Does this request require the capability '{c}'?"}
    caps = CAPABILITY_CATALOG
    t0 = time.time()
    preds = []
    with open(split_path) as f:
        rows = [json.loads(l) for l in f if l.strip()]
    for i, r in enumerate(rows):
        ans = agent.system_one(r["state"], Q)["answers"]
        p = {"id": r.get("_meta", {}).get("row_id", f"row{i}")}
        a = ans["qos_profile"]
        p["qos_probabilities"] = a.get("probabilities", {})
        p["qos_choice"] = a.get("choice")
        p["needs_planning"] = float(ans["needs_planning"].get("noul", 0.0))
        p["complexity"] = ans["complexity"].get("probabilities", {})
        p["urgency"] = ans["urgency"].get("probabilities", {})
        p["caps"] = {c: float(ans[f"cap__{c}"].get("noul", 0.0)) for c in caps}
        preds.append(p)
        if (i + 1) % 200 == 0:
            log(f"[{tid}] {i+1}/{len(rows)} ({time.time()-t0:.0f}s)")
    with open(out_path, "w") as f:
        json.dump({"teacher": tid, "split": os.path.basename(split_path), "preds": preds}, f)
    log(f"[{tid}] {out_path} gravado ({time.time()-t0:.0f}s)")


import laya  # noqa: E402
for tid in TEACHERS:
    t0 = time.time()
    agent = laya.load(f"{ROOT}/teachers/{tid}")
    log(f"[{tid}] carregado em {time.time()-t0:.0f}s")
    for split in ("dev", "distill"):
        sp = f"{ROOT}/data/{split}.jsonl"
        if os.path.exists(sp):
            predict_split(agent, tid, sp, f"{ROOT}/teachers_preds/{tid}__{split}.json")
    del agent
    import gc
    gc.collect()
    torch.cuda.empty_cache()

log("PHASE1_DONE")
