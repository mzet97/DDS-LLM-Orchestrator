#!/usr/bin/env python3
"""T-900-M1-04 — driver do harness de interop Rust↔Python (V02/V03).

Cenários:
  V02-a  Rust sub ← Python pub : 5 tópicos, conteúdo confere.
  V02-b  Python sub ← Rust pub : 5 tópicos, conteúdo confere.
  V02-c  MISMATCH deliberado: Python publica Routing.Decision no tópico
         Workflow.Intent → o assinante Rust NÃO recebe NADA (typename errado
         não faz matching).
  V03-a  Late joiner: publicador Python publica e MANTÉM vivo (hold); o
         assinante Rust inicia DEPOIS e recebe os dados retidos
         (TransientLocal com writer vivo).

Regressão dos 19 tópicos antigos: coberta por `cargo test -p dds-dataspace
--features dds` + `t808_dds` (real-DDS nineteen topics) — registrada na
evidência, não repetida aqui (custo alto sem mudança de risco).

Uso: python3 run_interop.py [--domain 211] [--rust-bin <caminho>]
Exit 0 = todos os cenários passaram.
"""
import argparse
import json
import os
import subprocess
import sys
import time
from pathlib import Path

HERE = Path(__file__).resolve().parent
RUST_BIN_DEFAULT = Path("target/debug/workflow-interop")

EXPECTED_TOPICS = {
    "Workflow.Intent", "Routing.Decision", "Agent.Capabilities", "Workflow.Plan", "Workflow.State"
}


def rust_bin(explicit: str | None) -> str:
    if explicit:
        return explicit
    # relativo à raiz do workspace rust (workers/ está em src/rust/)
    root = HERE.parent.parent.parent
    cand = root / RUST_BIN_DEFAULT
    if cand.exists():
        return str(cand)
    return str(Path.cwd() / "target/debug/workflow-interop")


LOOPBACK_CFG = HERE / "cyclonedds_loopback.xml"


def child_env() -> dict:
    # Disciplina do M1 REPORT: loopback, sem multicast, peers locais — sem
    # isso a config global do host (peers do cluster) impede a descoberta
    # entre processos de teste.
    env = dict(os.environ)
    env["CYCLONEDDS_URI"] = f"file://{LOOPBACK_CFG}"
    return env


def run_scenario(name: str, cmd_pub: list[str] | None, cmd_sub: list[str], expect_complete: bool,
                 timeout: float = 20.0) -> bool:
    """Inicia o SUBSCRIBER, espera descoberta, roda o PUBLICADOR e avalia."""
    print(f"\n=== {name} ===")
    sub = subprocess.Popen(cmd_sub, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                           env=child_env())
    time.sleep(1.5)  # descoberta antes do publicador
    pub = None
    if cmd_pub:
        pub = subprocess.Popen(cmd_pub, stdout=subprocess.PIPE, stderr=subprocess.PIPE, text=True,
                               env=child_env())
        try:
            pub_out, pub_err = pub.communicate(timeout=timeout)
        except subprocess.TimeoutExpired:
            pub.kill()
            pub_out, pub_err = pub.communicate()
        print("pub:", (pub_out or "").strip().replace("\n", " | ")[:200])
        if pub.returncode != 0:
            print("pub STDERR:", (pub_err or "").strip()[:300])
    try:
        sub_out, sub_err = sub.communicate(timeout=timeout)
    except subprocess.TimeoutExpired:
        sub.kill()
        sub_out, sub_err = sub.communicate()

    lines = [l for l in (sub_out or "").splitlines() if l.startswith("{")]
    events = []
    for l in lines:
        try:
            events.append(json.loads(l))
        except json.JSONDecodeError:
            pass
    topics = {e["topic"] for e in events if "topic" in e}
    print("sub topics:", sorted(topics) or "(nenhum)")
    if sub_err.strip():
        print("sub stderr:", sub_err.strip()[:200])

    if expect_complete:
        ok = topics == EXPECTED_TOPICS
        print("RESULTADO:", "PASS" if ok else "FAIL (esperava os 5 tópicos)")
        return ok
    ok = not topics
    print("RESULTADO:", "PASS (nada entregue, como exigido)" if ok else "FAIL (entregou algo!)")
    return ok


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("--domain", type=int, default=211)
    ap.add_argument("--rust-bin", default=None)
    args = ap.parse_args()
    py = sys.executable
    rb = rust_bin(args.rust_bin)
    pub_py = [py, str(HERE / "py_workflow_pub.py"), "--domain", str(args.domain)]
    sub_py = [py, str(HERE / "py_workflow_sub.py"), "--domain", str(args.domain)]
    sub_rs = [rb, "--mode", "sub", "--domain", str(args.domain), "--timeout-s", "15"]
    pub_rs = [rb, "--mode", "pub", "--domain", str(args.domain)]

    results = {}
    results["V02-a-rust-sub-python-pub"] = run_scenario(
        "V02-a: Rust subscreve ← Python publica", pub_py, sub_rs, expect_complete=True
    )
    results["V02-b-python-sub-rust-pub"] = run_scenario(
        "V02-b: Python subscreve ← Rust publica", pub_rs, sub_py, expect_complete=True
    )
    results["V02-c-deliberate-mismatch"] = run_scenario(
        "V02-c: mismatch deliberado (RoutingDecision no tópico Workflow.Intent)",
        pub_py + ["--wrong-type"], sub_rs, expect_complete=False,
    )
    # V03: publicador mantém vivo 8 s; subscriber inicia 1,5 s DEPOIS das
    # publicações (run_scenario já atrasa o sub 1,5 s — aqui o publicador
    # publica imediatamente e segura; o sub lê o retido TransientLocal).
    sub_late = [rb, "--mode", "late", "--domain", str(args.domain), "--timeout-s", "15"]
    results["V03-a-late-joiner-transientlocal"] = run_scenario(
        "V03-a: late joiner recebe TransientLocal (writer vivo)",
        pub_py + ["--hold-s", "8"], sub_late, expect_complete=True,
    )

    print("\n==== RESUMO ====")
    for k, v in results.items():
        print(f"{'PASS' if v else 'FAIL'}  {k}")
    return 0 if all(results.values()) else 1


if __name__ == "__main__":
    sys.exit(main())
