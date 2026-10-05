"""TOST formal — H1 (T_extra, EXP1) sobre os pares MAF × DDS (dados coletados
em cond0/ e cond50/ do EXP1, n=30 pares por condição).

H1 (dissertação Tab.7/§3.7.2): TOST pareado com MEI ±20 ms sobre a diferença
de T_extra. Se o IC 90% da diferença estiver inteiramente FORA de ±MEI,
conclui-se NÃO-equivalência com a direção estimada (desfecho informativo).
Razões de vazão/latência (H2/EXP3) reportadas descritivamente com IC bootstrap.
"""

from __future__ import annotations

import glob
import json
import math

import numpy as np
from scipy import stats


def t_extra(out: str, system: str, delay_ms: int = 0) -> list[float]:
    vals = []
    for f in sorted(glob.glob(f"{out}/{system}/run*.json")):
        d = json.load(open(f))
        if system == "dds":
            # WF_RECORD: T_total = soma das latências de estágio; T_backend = 3×delay
            total = sum(s["latency_ms"] for s in d["stages"]) / 1000.0
            vals.append(total - 3 * delay_ms / 1000.0)
        else:
            vals.append(d["t_total_s"] - d["t_backend_sum_s"])
    return vals


def tost_paired(diff: np.ndarray, mei: float) -> dict:
    n = len(diff)
    mean = float(diff.mean())
    se = float(diff.std(ddof=1) / math.sqrt(n))
    df = n - 1
    t_lower = (mean + mei) / se  # H01: dif <= -MEI
    t_upper = (mean - mei) / se  # H02: dif >= +MEI
    p_lower = float(stats.t.sf(t_lower, df))
    p_upper = float(stats.t.cdf(t_upper, df))
    ci90 = stats.t.ppf(0.95, df) * se
    return {
        "n": n,
        "media_dif_ms": round(mean * 1000, 1),
        "se_ms": round(se * 1000, 1),
        "ci90_ms": (round((mean - ci90) * 1000, 1), round((mean + ci90) * 1000, 1)),
        "p_tost_lower": p_lower,
        "p_tost_upper": p_upper,
        "equivalente": p_lower < 0.05 and p_upper < 0.05,
        "nao_equivalente_decidido": min(p_lower, p_upper) < 0.05
        and max(p_lower, p_upper) >= 0.05,
    }


def main() -> None:
    base = "/var/mnt/HD1TB/tese/src/rust/specs/880-campanha/exp1"
    for cond, delay in (("cond0", 0), ("cond50", 50)):
        maf = np.array(t_extra(f"{base}/{cond}", "maf"))
        lg = np.array(t_extra(f"{base}/{cond}", "langgraph"))
        dds = np.array(t_extra(f"{base}/{cond}", "dds", delay))
        controle = np.array(t_extra(f"{base}/{cond}", "controle"))
        print(f"\n=== condição delay={delay} ms ===")
        print(f"pares n={len(maf)} | T_extra médio: controle={controle.mean()*1000:.1f}ms "
              f"langgraph={lg.mean()*1000:.1f}ms maf={maf.mean()*1000:.1f}ms dds={dds.mean()*1000:.1f}ms")
        for name, other in (("MAF", maf), ("LangGraph", lg)):
            diff = dds - other
            r = tost_paired(diff, mei=0.020)
            verdict = (
                "EQUIVALENTE (TOST rejeita não-equivalência)"
                if r["equivalente"]
                else ("NÃO-EQUIVALENTE decidida (diferença > MEI, direção confirmada)"
                      if r["nao_equivalente_decidido"]
                      else "inconclusivo (potência insuficiente)")
            )
            print(f"  DDS − {name}: Δmédio={r['media_dif_ms']}ms ± {r['se_ms']}ms (se) "
                  f"IC90={r['ci90_ms']} → {verdict} [p_lower={r['p_tost_lower']:.2e}, p_upper={r['p_tost_upper']:.2e}]")
        # Fração T_extra/T_total (EXP2-style, estimada com o LLM do EXP2 = 5313ms total)
        frac_dds = dds.mean() * 1000 / 5313.0
        print(f"  fração T_extra/T_total projetada (EXP2, T_total=5313ms): {frac_dds:.3f} (MEI 0,10)")


if __name__ == "__main__":
    main()
