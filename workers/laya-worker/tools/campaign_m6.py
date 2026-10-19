#!/usr/bin/env python3
"""T-900-M6-03 — CAMPANHA COMPLETA (4 braços, N=30) com LLM real.

Braços (PROTOCOL_M6): static/rules | laya | planner_only | laya+planner.
Fase 1 (qualidade, 1 pass no dataset completo 32 PT/EN): acurácia de
roteamento/planejamento + macro-F1 (braços classificadores); taxa de planos
VÁLIDOS pelo `dag_validate` Rust + nós médios + tokens (braços planner).
Fase 2 (latência, N=30 runs × subset fixo de 4 rows): latência de decisão
(+planejamento quando aplicável) por braço; P50/P95; outliers MAD>3.5
reportados sem remoção; tokens por chamada.

Disciplina: chamadas SEQUENCIAIS ao llama-server (-np 1 — B-BENCH); manifest
por run; nada é removido; números desfavoráveis reportados como são.
Uso: campaign_m6.py <outdir> [--n 30]
"""

import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

WORKER = Path(__file__).resolve().parents[1]
REPO = WORKER.parent.parent
sys.path.insert(0, str(WORKER / "src"))
sys.path.insert(0, str(WORKER / "tools"))

import evaluate as _ev  # noqa: E402
import llm_planner  # noqa: E402
from laya_worker.adapter import RealLayaAdapter  # noqa: E402
from laya_worker import rules_baseline  # noqa: E402
from laya_worker.questions import CAPABILITY_CATALOG  # noqa: E402

DAG_VALIDATE = REPO / "target" / "debug" / "dag_validate"
LATENCY_IDS = ["pt-002", "en-014", "pt-009", "en-009"]  # 2 simples + 2 planning

INTENT_JSON = {
    "request_id": "r", "workflow_id": "w", "client_id": "c", "schema_version": 1,
    "intent_revision": 1, "command": 0, "expected_plan_revision": 0, "prompt": "p",
    "created_at_ns": 1, "deadline_ns": 9999999999999999, "security_context_ref": "",
    "policy_revision": 1, "questions_revision": 1, "capability_catalog_revision": 1,
    "checkpoint_revision": "", "max_tokens": 65536, "max_cost_microunits": 10**9,
    "budget_unit": "u", "fallback_template_id": "", "payload_hash": "",
}


def validate_plan(nodes, planner_tokens) -> dict:
    payload = json.dumps({
        "intent": INTENT_JSON,
        "candidate": llm_planner.to_candidate(nodes, planner_tokens),
    })
    p = subprocess.run(
        [str(DAG_VALIDATE)], input=payload, capture_output=True, text=True, timeout=30
    )
    try:
        return json.loads(p.stdout)
    except json.JSONDecodeError:
        return {"valid": False, "reason": f"dag_validate saída inválida: {p.stdout[:80]}"}


def mad_outliers(xs):
    if len(xs) < 4:
        return []
    med = statistics.median(xs)
    mad = statistics.median([abs(x - med) for x in xs]) or 1e-9
    return [x for x in xs if abs(x - med) / mad > 3.5]


def pct(xs, p):
    xs = sorted(xs)
    return xs[min(len(xs) - 1, round(p / 100 * (len(xs) - 1)))] if xs else 0


def arm_decide(arm: str, adapter, prompt: str) -> dict:
    """Uma decisão do braço → {latency_ms, tokens, qos?, needs?, caps?, plan?}."""
    t0 = time.perf_counter()
    out = {"latency_ms": 0, "tokens": 0}
    if arm == "rules":
        p = rules_baseline.predict(prompt)
        out.update(qos=p.qos_profile, needs=p.needs_planning_probability >= 0.5,
                   caps={c: p.capability_probabilities.get(c, 0.0) for c in CAPABILITY_CATALOG})
    elif arm == "laya":
        a = adapter.decide(prompt, time.time_ns() + 60_000_000_000)
        out.update(qos=a.qos_profile, needs=a.needs_planning_probability >= 0.5,
                   caps=dict(a.capability_probabilities))
    elif arm == "planner_only":
        nodes, meta = llm_planner.plan_nodes(prompt)
        out.update(tokens=meta["completion_tokens"], plan=nodes, plan_meta=meta)
    elif arm == "laya_planner":
        a = adapter.decide(prompt, time.time_ns() + 60_000_000_000)
        out.update(qos=a.qos_profile, needs=a.needs_planning_probability >= 0.5,
                   caps=dict(a.capability_probabilities))
        if a.needs_planning_probability >= 0.5:
            nodes, meta = llm_planner.plan_nodes(prompt)
            out.update(tokens=meta["completion_tokens"], plan=nodes, plan_meta=meta)
    out["latency_ms"] = round((time.perf_counter() - t0) * 1000)
    return out


def main() -> int:
    outdir = Path(sys.argv[1])
    n = 30
    if "--n" in sys.argv:
        n = int(sys.argv[sys.argv.index("--n") + 1])
    outdir.mkdir(parents=True, exist_ok=True)
    rows = _ev.load()
    by_id = {r["id"]: r for r in rows}
    lat_rows = [by_id[i] for i in LATENCY_IDS]
    adapter = RealLayaAdapter()
    adapter.decide("warmup", time.time_ns() + 60_000_000_000)  # load fora da medição

    arms = ["rules", "laya", "planner_only", "laya_planner"]

    # ---------- Fase 1: qualidade (1 pass, dataset completo) ----------
    print("== FASE 1: qualidade (32 rows) ==", flush=True)
    quality = {}
    for arm in arms:
        qos_ok = needs_ok = 0
        per_row = []
        plans_valid = plans_total = 0
        plan_nodes = []
        planner_calls = 0
        for r in rows:
            d = arm_decide(arm, adapter, r["prompt"])
            if "qos" in d:
                qos_ok += d["qos"] == r["qos"]
                needs_ok += bool(d["needs"]) == r["needs_planning"]
            if "plan" in d and d["plan"]:
                planner_calls += 1
                v = validate_plan(d["plan"], d.get("tokens", 0))
                plans_total += 1
                plans_valid += bool(v["valid"])
                plan_nodes.append(len(d["plan"]))
                if not v["valid"]:
                    per_row.append({"id": r["id"], "invalid_reason": v.get("reason", "?")})
            elif arm == "planner_only":
                plans_total += 1
                per_row.append({"id": r["id"], "invalid_reason": "sem nós extraídos"})
            per_row.append({"id": r["id"], "ms": d["latency_ms"], "tokens": d["tokens"]})
        q = {"n": len(rows)}
        if arm in ("rules", "laya"):
            q["qos_accuracy"] = round(qos_ok / len(rows), 4)
            q["needs_planning_accuracy"] = round(needs_ok / len(rows), 4)
        if arm in ("planner_only", "laya_planner"):
            q["planner_invocations"] = planner_calls
            q["plan_validity"] = round(plans_valid / plans_total, 4) if plans_total else None
            q["plan_nodes_mean"] = round(statistics.mean(plan_nodes), 2) if plan_nodes else 0
        quality[arm] = q
        print(f"  {arm}: {q}", flush=True)
    (outdir / "QUALITY.json").write_text(
        json.dumps(quality, indent=2, ensure_ascii=False) + "\n", encoding="utf-8")

    # ---------- Fase 2: latência (N runs × 4 rows) ----------
    print(f"== FASE 2: latência (N={n} × {len(lat_rows)} rows) ==", flush=True)
    for i in range(n):
        manifest = {
            "run": i + 1, "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "git_head": git_head(),
            "arms": arms, "subset": LATENCY_IDS,
            "decision_model": "laya==0.4.10 ckpt 891102d3… (cpu)",
            "planner_model": "gemma-4-12b-it-UD-Q4_K_XL sha256 90fd944d… b11671 ROCm -np1",
            "threat": "planner via HTTP direto (hop DDS do gateway fora da latência)",
        }
        per_arm = {}
        for arm in arms:
            lats, toks = [], []
            for r in lat_rows:
                d = arm_decide(arm, adapter, r["prompt"])
                lats.append(d["latency_ms"])
                toks.append(d["tokens"])
            per_arm[arm] = {
                "latency_ms": lats, "tokens": toks,
                "mean_ms": round(statistics.mean(lats)),
            }
        (outdir / f"run-{i+1:02d}.json").write_text(
            json.dumps({"manifest": manifest, "arms": per_arm}, indent=1) + "\n",
            encoding="utf-8")
        print(f"  run {i+1}/{n}: " + " | ".join(
            f"{a}: {per_arm[a]['mean_ms']}ms" for a in arms), flush=True)

    # ---------- Agregado ----------
    agg = {"runs": n, "arms": {}}
    for arm in arms:
        runs = [json.loads(f.read_text()) for f in sorted(outdir.glob("run-*.json"))]
        lats = [x for r in runs for x in r["arms"][arm]["latency_ms"]]
        toks = [x for r in runs for x in r["arms"][arm]["tokens"] if x]
        agg["arms"][arm] = {
            "samples": len(lats),
            "p50_ms": round(pct(lats, 50)), "p95_ms": round(pct(lats, 95)),
            "mean_ms": round(statistics.mean(lats)),
            "outliers_mad_gt_3_5": len(mad_outliers(lats)),
            "planner_tokens_total": sum(toks) if toks else 0,
            "planner_tokens_mean": round(statistics.mean(toks), 1) if toks else 0,
        }
    (outdir / "AGGREGATE.json").write_text(json.dumps(agg, indent=2) + "\n", encoding="utf-8")
    print(json.dumps(agg["arms"], indent=1))
    return 0


def git_head() -> str:
    try:
        return subprocess.run(["git", "rev-parse", "HEAD"], capture_output=True,
                              text=True, cwd=REPO, check=True).stdout.strip()
    except Exception:
        return "unavailable"


if __name__ == "__main__":
    sys.exit(main())
