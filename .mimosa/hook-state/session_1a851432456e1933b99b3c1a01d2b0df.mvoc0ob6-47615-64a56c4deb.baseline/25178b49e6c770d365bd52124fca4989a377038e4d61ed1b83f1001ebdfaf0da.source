#!/usr/bin/env python3
"""VM-side T904 fase 2: dispara UM braço de treino (detached) e marca progresso.
Env vars: T904_ARM (a|b|c|d), T904_DATA, T904_EVAL, T904_BASE, T904_LOSS, T904_SEED, T904_OUT.
laya-train roda desanexado; o driver local faz polling de /status e baixa os artefatos.
"""
import json
import os
import subprocess
import time

ROOT = "/content/laya_t904"
arm = os.environ["T904_ARM"]
out = os.environ["T904_OUT"]
status = f"{ROOT}/models/{arm}_status.json"


def write_status(d):
    with open(status, "w") as f:
        json.dump(d, f)


if os.path.exists(f"{out}/train_report.json"):
    write_status({"arm": arm, "state": "done"})
    print("CHUNK_TRAIN_ALREADY_DONE", flush=True)
    raise SystemExit(0)

cmd = ("laya-train --data {data} --eval {ev} --base {base} --out {out} "
       "--device cuda --seed {seed} --epochs 4 --micro-batch 8 --grad-accum 8 "
       "--encoder-lr 2.5e-5 --head-lr 1e-4 --loss {loss}").format(
    data=os.environ["T904_DATA"], ev=os.environ["T904_EVAL"], base=os.environ["T904_BASE"],
    out=out, seed=os.environ["T904_SEED"], loss=os.environ["T904_LOSS"])
write_status({"arm": arm, "state": "launching", "cmd": cmd, "started_at": time.time()})
logf = open(f"{ROOT}/logs/train_{arm}.log", "w")
p = subprocess.Popen(cmd, shell=True, stdout=logf, stderr=subprocess.STDOUT,
                     start_new_session=True)
print(f"CHUNK_TRAIN_LAUNCHED arm={arm} pid={p.pid}", flush=True)
write_status({"arm": arm, "state": "running", "pid": p.pid, "cmd": cmd,
              "started_at": time.time()})
