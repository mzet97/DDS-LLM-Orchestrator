"""TOST formal — EXP2 (fração T_extra/T_total, MEI 0,10) e EXP3 (razão de
vazão E(c), MEI ±15%) — dados coletados em 2026-10-05.

EXP2: a fração f = T_extra/T_total é razão de médias de DUAS amostras
independentes (T_extra do DDS: EXP1 cond50, n=30; T_total com LLM real:
EXP2, n=30). IC por Fieller (razão de médias normais) + bootstrap como
cross-check. Equivalência (overhead materialmente irrelevante) exige
IC90 ⊆ [0, 0,10]; não-equivalência decidida exige IC90 inteiramente acima
de 0,10 (desfecho informativo: o overhead permanece material).

EXP3: E(c) = Throughput(c)/(c·Throughput(1)), Throughput = n/Σlatências
(loop fechado). IC90 por bootstrap (reamostragem dentro de cada braço).
Equivalência exige IC90 ⊆ [0,85; 1,15].
"""
from __future__ import annotations

import glob
import json
import math

import numpy as np
from scipy import stats

BASE = "/var/mnt/HD1TB/tese/src/rust/specs/880-campanha"
RNG = np.random.default_rng(20261005)


def dds_t_extra_s(delay_ms: int = 50) -> np.ndarray:
    vals = []
    for f in sorted(glob.glob(f"{BASE}/exp1/cond50/dds/run*.json")):
        d = json.load(open(f))
        total = sum(s["latency_ms"] for s in d["stages"]) / 1000.0
        vals.append(total - 3 * delay_ms / 1000.0)
    return np.array(vals)


def exp2_t_total_ms() -> np.ndarray:
    vals = []
    for f in sorted(glob.glob(f"{BASE}/exp2/exp2-*.json")):
        d = json.load(open(f))
        vals.append(sum(s["latency_ms"] for s in d["stages"]))
    return np.array(vals)


def fieller_ratio_ci(x: np.ndarray, y: np.ndarray, conf: float = 0.90) -> dict:
    """IC de Fieller para θ = μx/μy (amostras independentes) — raízes da
    quadrática (x̄−θȳ)² = t²(vx − 2θvxy + θ²vy) com vxy = 0."""
    n, m = len(x), len(y)
    xbar, ybar = x.mean(), y.mean()
    vx = x.var(ddof=1) / n
    vy = y.var(ddof=1) / m
    df = n + m - 2  # conservador p/ amostras independentes
    t = stats.t.ppf(1 - (1 - conf) / 2, df)
    theta = xbar / ybar
    a = ybar**2 - t**2 * vy
    b = -2.0 * xbar * ybar  # vxy = 0 (amostras independentes)
    c = xbar**2 - t**2 * vx
    g = t**2 * vy / ybar**2
    if a <= 0:  # denominador quase-nulo → Fieller ilegível
        return {"theta": theta, "ci": (float("nan"), float("nan")),
                "fieller_valido": False, "g": g}
    disc = b * b - 4 * a * c
    if disc < 0:
        return {"theta": theta, "ci": (float("nan"), float("nan")),
                "fieller_valido": False, "g": g}
    lo = (-b - math.sqrt(disc)) / (2 * a)
    hi = (-b + math.sqrt(disc)) / (2 * a)
    return {"theta": theta, "ci": (min(lo, hi), max(lo, hi)),
            "fieller_valido": True, "g": g}


def bootstrap_ratio_ci(x: np.ndarray, y: np.ndarray, b: int = 20000,
                       conf: float = 0.90) -> tuple[float, float]:
    rx = RNG.choice(x, size=(b, len(x)), replace=True).mean(axis=1)
    ry = RNG.choice(y, size=(b, len(y)), replace=True).mean(axis=1)
    r = rx / ry
    a = (1 - conf) / 2
    return tuple(np.percentile(r, [100 * a, 100 * (1 - a)]))


def verdict_ratio(theta: float, ci: tuple[float, float], mei_lo: float,
                  mei_hi: float) -> str:
    lo, hi = ci
    if not math.isfinite(lo):
        return "inconclusivo (Fieller inválido)"
    if lo >= mei_lo and hi <= mei_hi:
        return "EQUIVALENTE (IC90 dentro do MEI)"
    if lo > mei_hi:
        return "NÃO-EQUIVALENTE decidida (IC90 inteiramente acima do MEI — desfecho informativo)"
    if hi < mei_lo:
        return "NÃO-EQUIVALENTE decidida (IC90 inteiramente abaixo do MEI — desfecho informativo)"
    return "inconclusivo (IC90 cruza o MEI — potência insuficiente)"


def main() -> None:
    # ---------- EXP2: fração T_extra/T_total ----------
    tex_s = dds_t_extra_s()               # segundos
    tex_ms = tex_s * 1000.0
    ttot_ms = exp2_t_total_ms()
    print("=== EXP2: fração T_extra/T_total (MEI [0; 0,10]) ===")
    print(f"T_extra DDS (EXP1 cond50): n={len(tex_ms)} média={tex_ms.mean():.0f}ms "
          f"dp={tex_ms.std(ddof=1):.0f}ms")
    print(f"T_total EXP2 (LLM real):   n={len(ttot_ms)} média={ttot_ms.mean():.0f}ms "
          f"dp={ttot_ms.std(ddof=1):.0f}ms")
    f = fieller_ratio_ci(tex_ms, ttot_ms)
    print(f"razão pontual f = {f['theta']:.4f} | Fieller IC90 = "
          f"[{f['ci'][0]:.4f}; {f['ci'][1]:.4f}] (g={f['g']:.4f})")
    bci = bootstrap_ratio_ci(tex_ms, ttot_ms)
    print(f"bootstrap IC90 (cross-check) = [{bci[0]:.4f}; {bci[1]:.4f}]")
    print(f"decisão: {verdict_ratio(f['theta'], f['ci'], 0.0, 0.10)}")

    # ---------- EXP3: razão de vazão E(c) ----------
    print("\n=== EXP3: razão de vazão E(c) (MEI [0,85; 1,15]) ===")
    lat = {}
    for c in (1, 2, 4, 8):
        vals = [int(x) for x in
                open(f"{BASE}/exp3/c{c}-latencies.txt").read().split()]
        lat[c] = np.array(vals, dtype=float)
        print(f"c={c}: n={len(vals)} p50={np.percentile(lat[c], 50):.0f}ms")

    # Modelo de reconstrução: blocos de c workflows em ordem (o arquivo é
    # append sequencial por bloco); wall do bloco ≈ max das latências do
    # bloco (loop fechado, c concorrentes). EXCLUI o spawn de processo +
    # discovery DDS do wf-run (~0,3–0,4 s/bloco) que o wall-clock congelado
    # do run_exp3.sh incluiu — por isso o modelo é o limite SUPERIOR da
    # vazão; o ponto congelado (0,6/1,2/2,3/4,1 wf/s → E(8)=0,85) é o
    # limite conservativo com spawn cobrado. Ambos reportados.
    def block_walls(v: np.ndarray, c: int) -> np.ndarray:
        return np.array([v[i:i + c].max() for i in range(0, len(v), c)])

    def tp_from_walls(v: np.ndarray, c: int) -> float:
        walls = block_walls(v, c)
        return len(v) / walls.sum() * 1000.0  # workflows/s

    b = 20000
    tp1_model = tp_from_walls(lat[1], 1)
    walls1 = lat[1]  # c=1: bloco = 1 valor
    for c in (2, 4, 8):
        ec = tp_from_walls(lat[c], c) / (c * tp1_model)
        # bootstrap de BLOCOS em cada braço (preserva a correlação intra-bloco
        # do max): wall do braço = soma dos maxes de blocos reamostrados
        wallsc = block_walls(lat[c], c)
        idx1 = RNG.integers(0, len(walls1), size=(b, len(walls1)))
        idxc = RNG.integers(0, len(wallsc), size=(b, len(wallsc)))
        tp1b = len(lat[1]) / walls1[idx1].sum(axis=1) * 1000.0
        tpcb = len(lat[c]) / wallsc[idxc].sum(axis=1) * 1000.0
        ecb = tpcb / (c * tp1b)
        ci = tuple(np.percentile(ecb, [5, 95]))
        print(f"E({c}) modelo (sem spawn) = {ec:.3f} | bootstrap IC90 = "
              f"[{ci[0]:.3f}; {ci[1]:.3f}] → {verdict_ratio(ec, ci, 0.85, 1.15)}")
    tp_frozen = {1: 0.6, 2: 1.2, 4: 2.3, 8: 4.1}  # wall-clock congelado (RESULTS)
    print(f"E(8) congelado c/ spawn cobrado = "
          f"{tp_frozen[8] / (8 * tp_frozen[1]):.3f} (ponto; precisão de 1 dígito "
          f"publicada no RESULTS)")
    print("nota: a decisão formal usa o modelo por bloco; o ponto congelado "
          "inclui overhead do harness (spawn+discovery por wf-run) e é reportado "
          "como limite conservativo.")


if __name__ == "__main__":
    main()
