"""Análise da campanha (T-907-10 / REQ-907-011): percentis, TOST, Hodges–Lehmann.

Protocolo congelado 860: TOST (α=0,05, IC 90%), Hodges–Lehmann, MEIs da Tab. 7
da dissertação (±20 ms latência; ±0,10 fração T_extra; ±15% vazão; ±20% p95).
Veredicto honesto: equivalente / NÃO-equivalente / inconclusivo — nunca "venceu".
"""

from __future__ import annotations

import argparse
import json
import math
from pathlib import Path


def load_totals(jsonl_path: str | Path, include_warmup: bool = False) -> list[float]:
    out = []
    for line in Path(jsonl_path).read_text().splitlines():
        if not line.strip():
            continue
        rec = json.loads(line)
        if rec.get("status") != "completed":
            continue
        if rec.get("warmup") and not include_warmup:
            continue
        if "t_total_s" in rec:
            out.append(float(rec["t_total_s"]))
        elif "makespan_s" in rec:
            out.append(float(rec["makespan_s"]))
    return out


def percentile(xs: list[float], q: float) -> float:
    xs = sorted(xs)
    if not xs:
        return math.nan
    k = (len(xs) - 1) * q
    f, c = math.floor(k), math.ceil(k)
    if f == c:
        return xs[int(k)]
    return xs[f] + (xs[c] - xs[f]) * (k - f)


def hodges_lehmann_paired(diffs: list[float]) -> float:
    """HL da mudança locacional pareada: mediana de (d_i + d_j)/2, i ≤ j."""
    n = len(diffs)
    if n == 0:
        return math.nan
    avgs = sorted((diffs[i] + diffs[j]) / 2.0 for i in range(n) for j in range(i, n))
    m = len(avgs)
    return avgs[m // 2] if m % 2 else (avgs[m // 2 - 1] + avgs[m // 2]) / 2.0


def tost_paired(a: list[float], b: list[float], mei: float,
                alpha: float = 0.05) -> dict:
    """TOST pareado por índice de run (interleave = bloco térmico comum).

    Equivalentes ⟺ IC(1−2α) da média das diferenças dentro de [−mei, +mei].
    """
    from scipy import stats

    n = min(len(a), len(b))
    if n < 2:
        return {"verdict": "inconclusivo", "n": n,
                "detail": "n < 2 (runs emparelháveis insuficientes)"}
    d = [a[i] - b[i] for i in range(n)]
    mean = sum(d) / n
    sd = math.sqrt(sum((x - mean) ** 2 for x in d) / (n - 1))
    se = sd / math.sqrt(n)
    if se > 0:
        tcrit = stats.t.ppf(1 - alpha, n - 1)
        lo, hi = mean - tcrit * se, mean + tcrit * se
        t_lo = (mean + mei) / se
        t_hi = (mean - mei) / se
        p = max(stats.t.sf(t_lo, n - 1), stats.t.cdf(t_hi, n - 1))
        equivalent = (lo >= -mei) and (hi <= mei)
    else:
        # Variância zero: o dado MAIS forte — IC degenerado no ponto.
        lo = hi = mean
        equivalent = abs(mean) <= mei
        p = 0.0 if equivalent else 1.0
    verdict = "equivalente" if equivalent else "NAO-equivalente"
    return {"verdict": verdict, "n": n, "mean_delta": mean, "sd_delta": sd,
            "ci90_low": lo, "ci90_high": hi, "mei": mei,
            "hodges_lehmann": hodges_lehmann_paired(d), "p_tost": p}


def summarize(jsonl: str | Path) -> dict:
    xs = load_totals(jsonl)
    return {"file": str(jsonl), "n": len(xs),
            "p50_s": percentile(xs, 0.50), "p95_s": percentile(xs, 0.95),
            "mean_s": sum(xs) / len(xs) if xs else math.nan}


def main():
    p = argparse.ArgumentParser(prog="baselines.analysis")
    p.add_argument("--baseline", required=True, help="JSONL do sistema de referência (ex. maf)")
    p.add_argument("--comparison", required=True, help="JSONL do sistema comparado (ex. dds)")
    p.add_argument("--mei-s", type=float, default=0.020,
                   help="MEI da Tab. 7: ±20 ms em escala de segundos (default 0.020)")
    p.add_argument("--alpha", type=float, default=0.05)
    p.add_argument("--json-out", default="")
    args = p.parse_args()

    s_base, s_comp = summarize(args.baseline), summarize(args.comparison)
    a = load_totals(args.comparison)
    b = load_totals(args.baseline)
    tost = tost_paired(a, b, mei=args.mei_s, alpha=args.alpha)
    report = {"baseline": s_base, "comparison": s_comp, "tost": tost,
              "mei_s": args.mei_s, "alpha": args.alpha,
              "nota": "veredicto de equivalência (TOST), não de superioridade"}
    print(json.dumps(report, ensure_ascii=False, indent=2))
    if args.json_out:
        Path(args.json_out).write_text(json.dumps(report, ensure_ascii=False, indent=2))


if __name__ == "__main__":
    main()
