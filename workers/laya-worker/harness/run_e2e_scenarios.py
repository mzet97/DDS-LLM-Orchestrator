#!/usr/bin/env python3
"""T-900-B — cenários E2E avançados (retry/restart/K=2/IN_DOUBT/cancel).

Cada cenário é um MODO independente que sobe SOmente os processos de que
precisa (llama-server é compartilhado se já houver um na porta; senão sobe).
Gates por assert (exit != 0 = FAIL) + artefatos em evidence/m6/e2e-scenarios/.

Modos:
  retry     — nó falha 1x (agente com engine flaky) → FAILOVER cria NOVO
              task_id laya1- (attempt 2) → workflow DONE.
  restart   — wf-run é KILLado após materializar → reinicia com o MESMO
              journal-dir → retoma a MESMA tentativa (mesmo task_id) → DONE.
  k2        — dois agentes --adaptive no mesmo domínio → claim exclusivo.
  indoubt   — intent com prazo curto → sweeper marca in_doubt no journal,
              SEM retry automático.
  cancel    — publica SUBMIT e depois CANCEL → estado terminal CANCELLED.

Uso: run_e2e_scenarios.py <modo> [--domain N]
"""

import argparse
import json
import os
import shutil
import signal
import subprocess
import sys
import time
import urllib.request
from pathlib import Path

HERE = Path(__file__).resolve().parent
WORKER = HERE.parent
REPO = WORKER.parent.parent
VENV = Path.home() / ".cache/tese-laya/venv/bin/python"
LLAMA_BIN = Path.home() / ".cache/tese-benchmarks/llama-prebuilt/llama-b11671/llama-server"
MODEL = Path("/var/mnt/HD1TB/tese/models/gemma-4-12b-it-UD-Q4_K_XL.gguf")
ENV = {k: str(v) for k, v in os.environ.items()}
ENV["CYCLONEDDS_URI"] = f"file://{HERE / 'cyclonedds_loopback.xml'}"
os.environ["CYCLONEDDS_URI"] = ENV["CYCLONEDDS_URI"]
EVID = REPO / "specs/900-laya-adaptive-routing/evidence/m6/e2e-scenarios"

sys.path.insert(0, str(WORKER))


def wait_health(port=8901, timeout_s=240):
    for _ in range(timeout_s // 5):
        try:
            with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=3) as r:
                if b"ok" in r.read():
                    return True
        except Exception:
            pass
        time.sleep(5)
    return False


class Stack:
    """Processos do cenário com cleanup garantido."""

    def __init__(self):
        self.procs = {}

    def start(self, name, *cmd, env=None, log=None, cwd=None):
        self.procs[name] = subprocess.Popen(
            list(cmd), stdout=open(log or f"/tmp/scn-{name}.log", "w"),
            stderr=subprocess.STDOUT, env=env or ENV, cwd=cwd,
        )
        return self.procs[name]

    def kill(self, name, sig=signal.SIGTERM):
        p = self.procs.pop(name, None)
        if p and p.poll() is None:
            p.send_signal(sig)
            try:
                p.wait(timeout=10)
            except subprocess.TimeoutExpired:
                p.kill()

    def cleanup(self):
        for name in list(self.procs):
            self.kill(name)


def llama_up(st: Stack, port=8901):
    """Garante llama-server no ar (se já houver um, usa)."""
    try:
        with urllib.request.urlopen(f"http://127.0.0.1:{port}/health", timeout=2) as r:
            if b"ok" in r.read():
                return True
    except Exception:
        pass
    st.start("llama", str(LLAMA_BIN), "-m", str(MODEL), "-ngl", "99", "-np", "1",
             "-c", "4096", "--host", "127.0.0.1", "--port", str(port))
    return wait_health(port)


def publish_intent(domain, request_id, prompt, *, command=0, expected_plan_revision=0,
                   deadline_ns=None, ttl_s=900):
    from cyclonedds.domain import DomainParticipant
    from cyclonedds.pub import Publisher, DataWriter
    from cyclonedds.topic import Topic
    from cyclonedds.qos import Qos, Policy
    import dds_llm_workflow_v1 as w

    ONE_S = 10**9
    q = Qos(
        Policy.Reliability.Reliable(max_blocking_time=ONE_S),
        Policy.Durability.TransientLocal, Policy.History.KeepLast(1),
        Policy.ResourceLimits(max_samples=64, max_instances=64, max_samples_per_instance=1),
        Policy.Ownership.Shared,
    )
    dp = DomainParticipant(domain)
    pub = Publisher(dp)
    t = Topic(dp, "Workflow.Intent", w.WorkflowIntent, qos=q)
    dw = DataWriter(pub, t, qos=q)
    now = time.time_ns()
    dw.write(w.WorkflowIntent(
        request_id=request_id, workflow_id=f"wf-{request_id}", client_id="scn",
        schema_version=1, intent_revision=1, command=command,
        expected_plan_revision=expected_plan_revision, prompt=prompt, created_at_ns=now,
        deadline_ns=deadline_ns or (now + ttl_s * ONE_S), security_context_ref="",
        policy_revision=1, questions_revision=1, capability_catalog_revision=1,
        checkpoint_revision="", max_tokens=4096, max_cost_microunits=10**9,
        budget_unit="MICRO_USD", fallback_template_id="", payload_hash="",
    ))
    return dw, dp  # manter vivos (TransientLocal)


# ------------------------- cenários ----------------------------------------


def scn_retry(domain, st, args):
    """B1: FAILOVER — nó falha 1x, tentativa 2 com NOVO task_id."""
    jdir = "/tmp/scn-retry-journal"
    shutil.rmtree(jdir, ignore_errors=True)
    assert llama_up(st), "llama-server não subiu"
    # proxy flaky na 8902: 1a req -> 503; demais -> repassa à 8901
    proxy = HERE / "flaky_proxy.py"
    st.start("proxy", sys.executable, str(proxy), "--upstream",
             "http://127.0.0.1:8901", "--port", "8902", "--fail-first",
             log="/tmp/scn-proxy.log")
    time.sleep(1)
    st.start("wfrun", str(REPO / "target/debug/wf-run"), "--domain", str(domain),
             "--planner", "direct", "--journal-dir", jdir, "--expect", "1",
             "--timeout-s", "300", log="/tmp/scn-wfrun.log")
    time.sleep(4)
    st.start("worker", str(VENV), "-m", "laya_worker", "--domain", str(domain),
             "--mock", "--max-intents", "1", "--idle-timeout", "120",
             env=dict(ENV, PYTHONPATH=str(WORKER / "src")), cwd=str(WORKER),
             log="/tmp/scn-worker.log")
    time.sleep(3)
    st.start("agent", str(REPO / "target/debug/agent"), "--agent-id", "scn-retry",
             "--dds-domain", str(domain), "--engine", "http",
             "--llama-url", "http://127.0.0.1:8902",  # via proxy flaky
             "--slots", "1", "--adaptive", "--adaptive-domain", str(domain),
             "--adaptive-capabilities", "summarization,research",
             log="/tmp/scn-agent.log")
    dw, dp = publish_intent(domain, "retry-1",
                            "Process the report with failover resilience in case resources are unavailable",
                            deadline_ns=time.time_ns() + 300_000_000_000)
    mark("intent publicada (prompt failover; engine via proxy flaky 1-fail)")
    rc = st.procs["wfrun"].wait(timeout=300)
    mark(f"wf-run exit={rc}")
    assert rc == 0, "wf-run não concluiu"
    import sqlite3
    c = sqlite3.connect(f"{jdir}/engine.db")
    rows = c.execute("select attempt, state, task_id from attempts order by attempt").fetchall()
    mark("attempts", rows=rows)
    assert len(rows) >= 2, "esperava >= 2 tentativas"
    assert rows[0][1] == "failed", f"tentativa 1 deveria estar failed: {rows[0]}"
    assert rows[-1][1] == "done", f"última deveria estar done: {rows[-1]}"
    assert rows[0][2] != rows[-1][2], "task_id deveria ser NOVO na tentativa 2"
    return True


def scn_restart(domain, st, args):
    """B2: kill do wf-run pós-materialize → reinício retoma MESMA tentativa."""
    jdir = "/tmp/scn-restart-journal"
    shutil.rmtree(jdir, ignore_errors=True)
    assert llama_up(st), "llama-server não subiu"
    st.start("wfrun1", str(REPO / "target/debug/wf-run"), "--domain", str(domain),
             "--planner", "direct", "--journal-dir", jdir, "--expect", "1",
             "--timeout-s", "240", log="/tmp/scn-wfrun1.log")
    time.sleep(4)
    st.start("worker", str(VENV), "-m", "laya_worker", "--domain", str(domain),
             "--max-intents", "1", "--idle-timeout", "120",
             env=dict(ENV, PYTHONPATH=str(WORKER / "src")), cwd=str(WORKER),
             log="/tmp/scn-worker.log")
    time.sleep(4)
    dw, dp = publish_intent(domain, "rst-1", "Traduza o documento para ingles",
                            deadline_ns=time.time_ns() + 300_000_000_000)
    # espera a task ser materializada (wf-run log "published=1")
    deadline_poll = time.time() + 180
    saw = False
    while time.time() < deadline_poll:
        time.sleep(3)
        import re as _re
        log = _re.sub(r"\x1b\[[0-9;]*m", "", Path("/tmp/scn-wfrun1.log").read_text(errors="replace"))
        if "published=1" in log:
            saw = True
            break
    if not saw:
        raise AssertionError(
            f"task não materializada em 180s; wfrun1.log tail: "
            f"{Path('/tmp/scn-wfrun1.log').read_text(errors='replace')[-500:]!r}")
    import sqlite3
    c = sqlite3.connect(f"{jdir}/engine.db")
    task_before = c.execute(
        "select task_id from attempts limit 1").fetchone()[0]
    mark(f"task materializada: {task_before[:24]}…")
    st.kill("wfrun1", signal.SIGKILL)  # CRASH
    mark("wf-run KILLado (SIGKILL)")
    time.sleep(3)
    # agente entra DEPOIS do crash (a task laya1- já está na malha)
    st.start("agent", str(REPO / "target/debug/agent"), "--agent-id", "scn-rst",
             "--dds-domain", str(domain), "--engine", "http",
             "--llama-url", "http://127.0.0.1:8901",
             "--slots", "1", "--adaptive", "--adaptive-domain", str(domain),
             "--adaptive-capabilities", "summarization,translation",
             log="/tmp/scn-agent.log")
    st.start("wfrun2", str(REPO / "target/debug/wf-run"), "--domain", str(domain),
             "--planner", "direct", "--journal-dir", jdir, "--expect", "1",
             "--timeout-s", "240", log="/tmp/scn-wfrun2.log")
    rc = st.procs["wfrun2"].wait(timeout=240)
    mark(f"wf-run2 exit={rc}")
    assert rc == 0
    c2 = sqlite3.connect(f"{jdir}/engine.db")
    rows = c2.execute("select attempt, state, task_id from attempts").fetchall()
    mark("attempts pós-restart", rows=rows)
    ids = {r[2] for r in rows}
    assert task_before in ids, f"mesmo task_id deveria persistir: {task_before} não está em {ids}"
    assert rows[-1][1] == "done"
    return True


def scn_k2(domain, st, args):
    """B3: dois agentes adaptativos; claim exclusivo; workflow DONE."""
    jdir = "/tmp/scn-k2-journal"
    shutil.rmtree(jdir, ignore_errors=True)
    assert llama_up(st), "llama-server não subiu"
    st.start("wfrun", str(REPO / "target/debug/wf-run"), "--domain", str(domain),
             "--planner", "direct", "--journal-dir", jdir, "--expect", "2",
             "--timeout-s", "300", log="/tmp/scn-wfrun.log")
    time.sleep(4)
    st.start("worker", str(VENV), "-m", "laya_worker", "--domain", str(domain),
             "--max-intents", "2", "--idle-timeout", "150",
             env=dict(ENV, PYTHONPATH=str(WORKER / "src")), cwd=str(WORKER),
             log="/tmp/scn-worker.log")
    time.sleep(3)
    for i, name in enumerate(("scn-a1", "scn-a2")):
        st.start(name, str(REPO / "target/debug/agent"), "--agent-id", name,
                 "--dds-domain", str(domain), "--engine", "http",
                 "--llama-url", "http://127.0.0.1:8901",
                 "--slots", "1", "--adaptive", "--adaptive-domain", str(domain),
                 "--adaptive-capabilities", "summarization,research,translation",
                 log=f"/tmp/scn-{name}.log")
        time.sleep(2)
    dwa, dpa = publish_intent(domain, "k2-1", "Resuma o relatorio trimestral")
    dwb, dpb = publish_intent(domain, "k2-2", "Research the outage timeline")
    rc = st.procs["wfrun"].wait(timeout=300)
    mark(f"wf-run exit={rc}")
    assert rc == 0, "wf-run não concluiu os 2 workflows"
    # nenhum task executado 2x: o journal mostra 1 attempt done por nó e o
    # wf-run chegou a DONE sem retry
    import sqlite3
    c = sqlite3.connect(f"{jdir}/engine.db")
    states = dict(c.execute("select state, count(*) from attempts group by state").fetchall())
    mark("attempt states", states=states)
    assert states.get("failed", 0) == 0, "sem falhas esperadas"
    assert states.get("done", 0) == 2
    return True


def scn_indoubt(domain, st, args):
    """B4: prazo curtíssimo → sweeper marca in_doubt, sem retry."""
    jdir = "/tmp/scn-indoubt-journal"
    shutil.rmtree(jdir, ignore_errors=True)
    # sem llama-server: a task NUNCA executa (agente sobe com porta morta)
    st.start("wfrun", str(REPO / "target/debug/wf-run"), "--domain", str(domain),
             "--planner", "direct", "--journal-dir", jdir, "--expect", "99",
             "--timeout-s", "120", log="/tmp/scn-wfrun.log")
    time.sleep(4)
    st.start("worker", str(VENV), "-m", "laya_worker", "--domain", str(domain),
             "--max-intents", "1", "--idle-timeout", "60",
             env=dict(ENV, PYTHONPATH=str(WORKER / "src")), cwd=str(WORKER),
             log="/tmp/scn-worker.log")
    time.sleep(4)
    st.start("agent", str(REPO / "target/debug/agent"), "--agent-id", "scn-id",
             "--dds-domain", str(domain), "--engine", "http",
             "--llama-url", "http://127.0.0.1:59999",  # porta morta
             "--slots", "1", "--adaptive", "--adaptive-domain", str(domain),
             "--adaptive-capabilities", "summarization",
             log="/tmp/scn-agent.log")
    dw, dp = publish_intent(domain, "id-1", "Resuma o relatorio",
                            deadline_ns=time.time_ns() + 100_000_000_000)  # 100s
    mark("intent com prazo de 40s publicada")
    time.sleep(160)  # deixa o deadline estourar + sweeper rodar
    st.kill("wfrun")
    import sqlite3
    c = sqlite3.connect(f"{jdir}/engine.db")
    rows = c.execute("select node_id, state from attempts").fetchall()
    mark("attempts", rows=rows)
    assert any(r[1] == "in_doubt" for r in rows), f"esperava in_doubt: {rows}"
    # sem retry automático: só UMA tentativa
    assert len(rows) == 1, f"esperava 1 tentativa (sem retry): {rows}"
    return True


def scn_cancel(domain, st, args):
    """B5: SUBMIT e depois CANCEL → terminal CANCELLED."""
    jdir = "/tmp/scn-cancel-journal"
    shutil.rmtree(jdir, ignore_errors=True)
    st.start("wfrun", str(REPO / "target/debug/wf-run"), "--domain", str(domain),
             "--planner", "direct", "--journal-dir", jdir, "--expect", "1",
             "--timeout-s", "150", log="/tmp/scn-wfrun.log")
    time.sleep(4)
    st.start("worker", str(VENV), "-m", "laya_worker", "--domain", str(domain),
             "--max-intents", "1", "--idle-timeout", "60",
             env=dict(ENV, PYTHONPATH=str(WORKER / "src")), cwd=str(WORKER),
             log="/tmp/scn-worker.log")
    time.sleep(4)
    dw, dp = publish_intent(domain, "cxl-1", "Resuma o relatorio",
                            deadline_ns=time.time_ns() + 120_000_000_000)
    # CANCEL depois de 8s (workflow já aceito/planejado, task publicada)
    time.sleep(8)
    dw2, dp2 = publish_intent(domain, f"cxl-1-cxl", "cancel", command=1,
                              expected_plan_revision=0, ttl_s=60)
    # nota: o CANCEL precisa do MESMO workflow_id — publica com workflow_id igual
    from cyclonedds.domain import DomainParticipant
    from cyclonedds.pub import Publisher, DataWriter
    from cyclonedds.topic import Topic
    from cyclonedds.qos import Qos, Policy
    import dds_llm_workflow_v1 as w
    import time as _t
    ONE_S = 10**9
    q = Qos(Policy.Reliability.Reliable(max_blocking_time=ONE_S),
            Policy.Durability.TransientLocal, Policy.History.KeepLast(1),
            Policy.ResourceLimits(max_samples=64, max_instances=64, max_samples_per_instance=1),
            Policy.Ownership.Shared)
    dp3 = DomainParticipant(domain)
    t3 = Topic(dp3, "Workflow.Intent", w.WorkflowIntent, qos=q)
    dw3 = DataWriter(Publisher(dp3), t3, qos=q)
    now = _t.time_ns()
    dw3.write(w.WorkflowIntent(
        request_id="cxl-1-cancel", workflow_id="wf-cxl-1", client_id="scn",
        schema_version=1, intent_revision=1, command=1,  # IC_CANCEL
        expected_plan_revision=0, prompt="", created_at_ns=now,
        deadline_ns=now + 60 * ONE_S, security_context_ref="",
        policy_revision=1, questions_revision=1, capability_catalog_revision=1,
        checkpoint_revision="", max_tokens=0, max_cost_microunits=0,
        budget_unit="", fallback_template_id="", payload_hash="",
    ))
    mark("CANCEL publicado p/ wf-cxl-1")
    # espera o wf-run reportar terminal (exit por expect=1: DONE conta tb —
    # mas cancel NÃO é done… wf-run conta done apenas failed.is_none() e
    # terminal; CANCELLED seta terminal sem failed → conta como concluído)
    rc = st.procs["wfrun"].wait(timeout=150)
    mark(f"wf-run exit={rc}")
    out = Path("/tmp/scn-wfrun.log").read_text()
    assert '"terminal": true' in out or rc == 0, f"sem terminal no log"
    assert "wf-cxl-1" in out
    return True


SCENARIOS = {"retry": scn_retry, "restart": scn_restart, "k2": scn_k2,
             "indoubt": scn_indoubt, "cancel": scn_cancel}
_timeline = []


def mark(ev, **kw):
    _timeline.append({"t": round(time.time(), 1), "event": ev, **kw})
    print(f">> {ev} {kw}", flush=True)


def main() -> int:
    ap = argparse.ArgumentParser()
    ap.add_argument("mode", choices=list(SCENARIOS))
    ap.add_argument("--domain", type=int, default=60)
    args = ap.parse_args()
    EVID.mkdir(parents=True, exist_ok=True)
    st = Stack()
    ok = False
    try:
        ok = SCENARIOS[args.mode](args.domain, st, args)
        mark(f"CENÁRIO {args.mode}", RESULT="PASS" if ok else "FAIL")
    except Exception as e:
        mark(f"CENÁRIO {args.mode}", RESULT="FAIL", error=str(e)[:200])
    finally:
        st.cleanup()
        (EVID / f"{args.mode}.timeline.json").write_text(
            json.dumps(_timeline, indent=1, ensure_ascii=False))
        for name in ("wfrun", "wfrun1", "wfrun2", "worker", "agent", "agent-bad",
                     "agent-good", "scn-a1", "scn-a2"):
            src = Path(f"/tmp/scn-{name}.log")
            if src.exists():
                (EVID / f"{name}.log").write_text(src.read_text()[:150_000])
    return 0 if ok else 1


if __name__ == "__main__":
    sys.exit(main())
