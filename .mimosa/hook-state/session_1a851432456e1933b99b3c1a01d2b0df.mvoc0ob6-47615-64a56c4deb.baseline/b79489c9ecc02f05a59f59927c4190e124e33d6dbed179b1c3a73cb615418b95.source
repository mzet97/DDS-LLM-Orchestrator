#!/usr/bin/env python3
"""T-900-M6-03 — PILOTO de campanha (braços de decisão, Laya REAL).

Protocolo PROTOCOL_M6 (congelado): braços static/rules e laya; N runs por
braço (piloto: N=5, campanha completa N>=30 após piloto); warmup 5 decisões
descartadas por braço; manifest por run (HEAD/lock/SDK/checkpoint/hardware/
timestamps); outliers MAD>3.5 reportados sem remoção. Braços planner-only e
laya+planner ficam PENDENTES de integração E2E do engine (documentado).

Uso: python3 pilot_m6.py <outdir> [--n 5]
"""

import json
import statistics
import subprocess
import sys
import time
from pathlib import Path

WORKER = Path(__file__).resolve().parents[1]
sys.path.insert(0, str(WORKER / "src"))
sys.path.insert(0, str(WORKER / "tools"))

import evaluate as _ev  # noqa: E402
from laya_worker.adapter import RealLayaAdapter  # noqa: E402
from laya_worker import rules_baseline  # noqa: E402


def mad_outliers(xs):
    if len(xs) < 4:
        return []
    med = statistics.median(xs)
    mad = statistics.median([abs(x - med) for x in xs]) or 1e-9
    return [x for x in xs if abs(x - med) / mad > 3.5]


def git_head() -> str:
    try:
        return subprocess.run(
            ["git", "rev-parse", "HEAD"], capture_output=True, text=True,
            cwd=WORKER.parent.parent, check=True,
        ).stdout.strip()
    except Exception:
        return "unavailable"


def run_arm(name: str, decide_fn, rows, warmup: int = 5) -> dict:
    # warmup descartado
    for r in rows[:warmup]:
        decide_fn(r["prompt"])
    lat = []
    qos_ok = 0
    for r in rows:
        t0 = time.perf_counter()
        pred = decide_fn(r["prompt"])
        lat.append((time.perf_counter() - t0) * 1000)
        qos_ok += pred == r["qos"]
    lat_sorted = sorted(lat)
    return {
        "arm": name,
        "n": len(rows),
        "qos_accuracy": round(qos_ok / len(rows), 4),
        "latency_ms": {
            "p50": round(lat_sorted[len(lat) // 2]),
            "p95": round(lat_sorted[min(len(lat) - 1, round(0.95 * (len(lat) - 1)))]),
            "mean": round(statistics.mean(lat), 1),
        },
        "outliers_mad_gt_3_5": [round(x, 1) for x in mad_outliers(lat)],
    }


def main() -> int:
    outdir = Path(sys.argv[1])
    n = 5
    if "--n" in sys.argv:
        n = int(sys.argv[sys.argv.index("--n") + 1])
    outdir.mkdir(parents=True, exist_ok=True)
    rows = _ev.load()

    adapter = RealLayaAdapter()
    # load antecipado (fora da medição)
    adapter.decide("warmup", time.time_ns() + 60_000_000_000)

    for i in range(n):
        manifest = {
            "run": i + 1,
            "utc": time.strftime("%Y-%m-%dT%H:%M:%SZ", time.gmtime()),
            "git_head": git_head(),
            "cargo_lock_sha256": _sha(WORKER.parent.parent / "Cargo.lock"),
            "idl_sha256": _sha(WORKER / "dds_llm_workflow_v1" / "_WorkflowPlanning.py"),
            "sdk": "laya==0.4.10 (Apache-2.0)",
            "checkpoint": "891102d372688fc2a094dac56a384bc537b87c63f21f9f3dac0be2b7cbc8d86c",
            "torch": "2.14.1+cpu",
            "hardware": "AMD Ryzen 9 5900X; decisão em CPU",
            "dataset": "dataset_v1_seed.jsonl (32 PT/EN)",
            "warmup_discarded": 5,
            "arms_pendentes": ["planner-only", "laya+planner (integração E2E do engine)"],
        }
        arms = [
            run_arm("static/rules", lambda p: rules_baseline.predict(p).qos_profile, rows),
            run_arm(
                "laya",
                lambda p: adapter.decide(p, time.time_ns() + 60_000_000_000).qos_profile,
                rows,
            ),
        ]
        report = {"manifest": manifest, "arms": arms}
        (outdir / f"run-{i+1:02d}.json").write_text(
            json.dumps(report, indent=2, ensure_ascii=False) + "\n", encoding="utf-8"
        )
        print(f"run {i+1}/{n}: " + " | ".join(
            f"{a['arm']}: qos={a['qos_accuracy']} p50={a['latency_ms']['p50']}ms" for a in arms
        ))

    # agregado
    agg = {"runs": n, "arms": {}}
    for arm_name in ("static/rules", "laya"):
        runs = [
            json.loads(f.read_text())
            for f in sorted(outdir.glob("run-*.json"))
        ]
        qs = [next(a for a in r["arms"] if a["arm"] == arm_name) for r in runs]
        agg["arms"][arm_name] = {
            "qos_accuracy_mean": round(statistics.mean(a["qos_accuracy"] for a in qs), 4),
            "p50_ms_mean": round(statistics.mean(a["latency_ms"]["p50"] for a in qs)),
            "p95_ms_max": max(a["latency_ms"]["p95"] for a in qs),
            "outliers_total": sum(len(a["outliers_mad_gt_3_5"]) for a in qs),
        }
    (outdir / "AGGREGATE.json").write_text(
        json.dumps(agg, indent=2) + "\n", encoding="utf-8"
    )
    print(json.dumps(agg["arms"], indent=1))
    return 0


def _sha(p: Path) -> str:
    import hashlib

    return hashlib.sha256(p.read_bytes()).hexdigest()[:16]


if __name__ == "__main__":
    sys.exit(main())
