#!/usr/bin/env python3
"""T904-M5 — driver LOCAL do Colab CLI (sessão laya905, T4, teto de custo).

Fases: session / upload-teachers / upload-data / teachers (inferência) / train-ARM /
fetch-ARM. Toda transferência grande é fatiada (21 MB) e conferida por SHA-256.
Teto: --cu-floor (default: para se o saldo ficar abaixo de inicial-20).
"""
import argparse
import hashlib
import json
import os
import re
import subprocess
import sys
import time

SESSION = "laya905"
PART = 21 * 1024 * 1024
ROOT = "/content/laya_t905"


def sh(cmd, timeout=600):
    return subprocess.run(cmd, shell=True, capture_output=True, text=True, timeout=timeout)


def colab(cmd, timeout=600, retries=3):
    last = None
    for i in range(retries):
        r = sh(f"colab {cmd}", timeout=timeout)
        out = r.stdout + r.stderr
        if r.returncode == 0:
            return out
        last = RuntimeError(f"colab {cmd} falhou (exit {r.returncode}):\n{out[-800:]}")
        if "500 Server Error" in out or "IncompleteRead" in out:
            time.sleep(20 * (i + 1))
            continue
        raise last
    raise last


def sha256(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for c in iter(lambda: f.read(1 << 20), b""):
            h.update(c)
    return h.hexdigest()


def usage():
    out = colab("usage")
    m = re.search(r"([\d.]+)\s*compute units", out)
    rate = re.search(r"([\d.]+)/h", out)
    return (float(m.group(1)) if m else -1.0), (rate.group(1) if rate else "?")


class Budget:
    def __init__(self, floor):
        self.floor = floor
        self.start = None

    def check(self, tag):
        bal, rate = usage()
        if self.start is None:
            self.start = bal
        print(f"[budget] {tag}: saldo={bal} CU (rate {rate}/h)", flush=True)
        with open(os.path.expanduser(
                "/var/mnt/HD1TB/tese/src/rust/specs/904-laya-unified-multiteacher/budget_usage.jsonl"), "a") as f:
            f.write(json.dumps({"t": time.strftime("%FT%T"), "tag": tag, "balance_cu": bal}) + "\n")
        if 0 <= bal < self.floor:
            raise SystemExit(f"TETO DE CUSTO ATINGIDO (saldo {bal} < {self.floor}) — salvar estado e relatar.")


def ensure_session():
    out = sh("colab status -s laya905").stdout + sh("colab status -s laya905").stderr
    if "No active" in out or "not found" in out.lower() or out.strip() == "":
        print("[session] criando laya905 (T4)…", flush=True)
        colab("new -s laya905 --gpu T4", timeout=900)
    else:
        print("[session] reutilizando laya905", flush=True)
    for _ in range(30):
        st = sh("colab status -s laya905").stdout
        if "running" in st.lower() or "active" in st.lower() or "Ready" in st:
            break
        time.sleep(10)
    print(exec_inline("print('SESSION_OK')", 120, 300).strip())



def exec_inline(code, timeout_s=300, hard=600):
    """exec -f com arquivo temporário (o CLI não tem -c)."""
    tmp = "/tmp/t904_inline.py"
    with open(tmp, "w") as f:
        f.write(code)
    return colab(f"exec -s {SESSION} -f {tmp} --timeout {timeout_s}", timeout=hard)


def upload_chunked(local, remote):
    """Fatia local, sobe partes, remonta na VM via exec."""
    size = os.path.getsize(local)
    if size <= PART:
        colab(f"upload -s {SESSION} '{local}' '{remote}'", timeout=1200)
        return
    parts_dir = local + ".parts"
    os.makedirs(parts_dir, exist_ok=True)
    names = []
    with open(local, "rb") as f:
        i = 0
        while True:
            b = f.read(PART)
            if not b:
                break
            n = f"p{i:02d}"
            with open(f"{parts_dir}/{n}", "wb") as g:
                g.write(b)
            names.append(n)
            i += 1
    exec_inline(f"import os; os.makedirs('{remote}.parts', exist_ok=True); "
                f"print('PARTS_DIR_OK', os.path.isdir('{remote}.parts'))")
    # resume: partes já presentes na VM com o mesmo tamanho são puladas
    local_sizes = {n: os.path.getsize(f"{parts_dir}/{n}") for n in names}
    listing = exec_inline(
        "import os, json; p=r'" + remote + ".parts'; "
        "print('EXISTING ' + json.dumps({f: os.path.getsize(os.path.join(p, f)) for f in os.listdir(p)} if os.path.isdir(p) else {}))",
        180, 300)
    existing = {}
    for line in listing.splitlines():
        if line.startswith("EXISTING "):
            existing = json.loads(line[len("EXISTING "):])
    for n in names:
        if existing.get(n) == local_sizes[n]:
            print(f"  skip {n} (já na VM)", flush=True)
            continue
        colab(f"upload -s {SESSION} '{parts_dir}/{n}' '{remote}.parts/{n}'", timeout=600)
        print(f"  upload {os.path.basename(local)}/{n} ({i} partes)", flush=True)
    script = (f"import glob,os,hashlib;parts=sorted(glob.glob('{remote}.parts/p*'));"
              f"os.makedirs(os.path.dirname('{remote}'),exist_ok=True);"
              f"h=hashlib.sha256();"
              f"open('{remote}','wb').write(b''.join(open(p,'rb').read() for p in parts));"
              f"h.update(open('{remote}','rb').read());print('ASM_SHA',h.hexdigest())")
    out = exec_inline(script, 600, 900)
    print("  ", out.strip().splitlines()[-1] if out.strip() else "asm?", flush=True)


def download_chunked(remote, local):
    """Pede à VM para fatiar, baixa as partes e remonta com SHA."""
    script = (f"import os,hashlib,math;P='{remote}';sz=os.path.getsize(P);"
              f"h=hashlib.sha256();h.update(open(P,'rb').read());"
              f"import shutil;os.system('rm -rf {remote}.dp');os.makedirs('{remote}.dp',exist_ok=True);"
              f"n=math.ceil(sz/{PART});"
              f"f=open(P,'rb');\n"
              f"for i in range(n):\n"
              f"    open(f'{{P}}.dp/p{{i:02d}}','wb').write(f.read({PART}));\n"
              f"f.close();print('DLINFO',h.hexdigest(),n,sz)")
    out = exec_inline(script, 600, 900)
    m = re.search(r"DLINFO ([0-9a-f]{64}) (\d+) (\d+)", out)
    assert m, f"sem DLINFO: {out[-800:]}"
    want, nparts, sz = m.group(1), int(m.group(2)), int(m.group(3))
    os.makedirs(os.path.dirname(local) or ".", exist_ok=True)
    with open(local, "wb") as g:
        for i in range(nparts):
            tp = f"{local}.dp{i:02d}"
            colab(f"download -s {SESSION} '{remote}.dp/p{i:02d}' '{tp}'", timeout=900)
            g.write(open(tp, "rb").read())
            os.remove(tp)
            print(f"  download {os.path.basename(remote)} p{i:02d}/{nparts}", flush=True)
    got = sha256(local)
    assert got == want, f"SHA divergente pós-download: {got} != {want} ({local})"
    assert os.path.getsize(local) == sz
    print(f"  SHA OK {got[:12]}… ({local}, {sz} B)", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("phase", choices=["session", "env", "upload-teachers", "upload-data",
                                      "teachers", "train", "poll", "fetch", "status"])
    ap.add_argument("--arm", help="a|b|c|d")
    ap.add_argument("--cu-floor", type=float, default=348.0)
    args = ap.parse_args()
    budget = Budget(args.cu_floor)

    if args.phase == "session":
        ensure_session()
        return 0

    if args.phase == "env":
        ensure_session()
        budget.check("pre-env")
        out = colab(f"exec -s {SESSION} -f /var/mnt/HD1TB/tese/src/laya_t900_handoff/t905/scripts/vm_t905_phase0_env.py --timeout 600", timeout=900)
        print(out[-3000:])
        return 0

    if args.phase == "upload-teachers":
        budget.check("pre-upload-teachers")
        tdir = "/home/mzet/.cache/tese-laya/checkpoints"
        pairs = (("vigente", "laya-production"),)
        comp_src = ("/home/mzet/.cache/huggingface/hub/models--convaiinnovations--laya/"
                    "snapshots/7b928d828b7b0e022f929d9bd2e44165aa270148")
        for tid, w in pairs or (("t900", "t900-specialized"), ("t902b", "t902-arm-b"), ("t903c", "t903-arm-c")):
            upload_chunked(f"{tdir}/{w}/model.safetensors", f"{ROOT}/upload/{tid}.parts/model.safetensors")
        exec_inline(f"import os;os.makedirs('{ROOT}/teachers/_complement',exist_ok=True)")
        for rel, flat in (("rl_agent_config.json", "rl_agent_config.json"),
                          ("encoder/config.json", "encoder_config.json"),
                          ("tokenizer/tokenizer_config.json", "tokenizer_tokenizer_config.json"),
                          ("tokenizer/tokenizer.json", "tokenizer_tokenizer.json")):
            colab(f"upload -s {SESSION} '{comp_src}/{rel}' '{ROOT}/teachers/_complement/{flat}'", timeout=600)
        print("professores enviados", flush=True)
        return 0

    if args.phase == "upload-data":
        budget.check("pre-upload-data")
        d = "/var/mnt/HD1TB/tese/src/laya_t900_handoff/t904/datasets/splits"
        exec_inline(f"import os;os.makedirs('{ROOT}/data',exist_ok=True);os.makedirs('{ROOT}/tools',exist_ok=True);os.makedirs('{ROOT}/logs',exist_ok=True);os.makedirs('{ROOT}/models',exist_ok=True)")
        for f in ("train.jsonl", "dev.jsonl", "calibration.jsonl"):
            p = f"{d}/{f}"
            if os.path.exists(p):
                colab(f"upload -s {SESSION} '{p}' '{ROOT}/data/{f}'", timeout=1800)
                print(f"  data/{f} ({os.path.getsize(p)//1024} KiB)", flush=True)
        colab(f"upload -s {SESSION} '/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/src/laya_worker/questions.py' '{ROOT}/tools/questions.py'", timeout=300)
        for s in ("vm_t905_phase1_teachers.py", "vm_t905_phase2_train.py", "vm_t905_eval.py"):
            colab(f"upload -s {SESSION} '/var/mnt/HD1TB/tese/src/laya_t900_handoff/t905/scripts/{s}' '{ROOT}/{s}'", timeout=300)
        print("dados/scripts enviados", flush=True)
        return 0

    if args.phase == "teachers":
        budget.check("pre-teachers")
        out = colab(f"exec -s {SESSION} -f /var/mnt/HD1TB/tese/src/laya_t900_handoff/t905/scripts/vm_t905_phase1_base.py --timeout 7200", timeout=7500)
        print(out[-4000:])
        return 0

    if args.phase == "train":
        assert args.arm, "--arm obrigatório"
        budget.check(f"pre-train-{args.arm}")
        cfg = {
            "n1": dict(data="train.jsonl", base=f"{ROOT}/teachers/vigente",
                       loss="soft-ce", seed="905"),
            "n2": dict(data="train.jsonl", base=f"{ROOT}/teachers/vigente",
                       loss="soft-ce", seed="906"),
        }[args.arm]
        env = (f"--env T904_ARM={args.arm} --env T904_DATA={ROOT}/data/{cfg['data']} "
               f"--env T904_EVAL={ROOT}/data/dev.jsonl --env T904_BASE={cfg['base']} "
               f"--env T904_LOSS={cfg['loss']} --env T904_SEED={cfg['seed']} "
               f"--env T904_OUT={ROOT}/models/arm_{args.arm}")
        out = colab(f"exec -s {SESSION} -f /var/mnt/HD1TB/tese/src/laya_t900_handoff/t905/scripts/vm_t905_phase2_train.py {env} --timeout 300", timeout=600)
        print(out[-1500:])
        return 0

    if args.phase == "poll":
        assert args.arm
        out = exec_inline(
            f"import json,os;p='{ROOT}/models/{args.arm}_status.json';"
            f"r='{ROOT}/models/arm_{args.arm}/train_report.json';"
            f"print(open(p).read() if os.path.exists(p) else 'no-status');"
            f"print('REPORT_YES' if os.path.exists(r) else 'REPORT_NO')", 120, 300)
        print(out.strip()[-600:])
        return 0

    if args.phase == "fetch":
        assert args.arm
        arm = args.arm
        odir = "/var/mnt/HD1TB/tese/src/laya_t900_handoff/t904/models"
        os.makedirs(odir, exist_ok=True)
        download_chunked(f"{ROOT}/models/arm_{arm}/model.safetensors", f"{odir}/arm_{arm}/model.safetensors")
        for f in ("train_report.json",):
            colab(f"download -s {SESSION} '{ROOT}/models/arm_{arm}/{f}' '{odir}/arm_{arm}/{f}'", timeout=600)
        colab(f"download -s {SESSION} '{ROOT}/logs/train_{arm}.log' '{odir}/arm_{arm}/train_{arm}.log'", timeout=600)
        print(f"arm_{arm} baixado", flush=True)
        return 0

    if args.phase == "status":
        bal, rate = usage()
        print(f"saldo={bal} CU rate={rate}/h")
        return 0
    return 0


if __name__ == "__main__":
    sys.exit(main())
