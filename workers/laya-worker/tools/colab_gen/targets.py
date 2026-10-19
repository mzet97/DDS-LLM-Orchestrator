"""Converte `expected` (rótulos) em vetores de probabilidade por pergunta.

A ordem dos marcadores internos do laya segue a ordem DECLARADA na pergunta:
- choice: ordem das chaves de `criteria`;
- noul: [não, sim] (2 markers);
- score: níveis 0..L-1 na ordem de `criteria`.
"""
import sys
from pathlib import Path

sys.path.insert(0, str(Path(__file__).resolve().parents[1] / "src"))
from laya_worker.questions import CAPABILITY_CATALOG, QOS_PROFILES  # noqa: E402

COMPLEXITY_LEVELS = 4
URGENCY_LEVELS = 3


def _one_hot(n, idx):
    v = [0.0] * n
    v[idx] = 1.0
    return v


def numeric_targets(questions: dict, expected: dict):
    out = {}
    for qid, q in questions.items():
        t = q.get("type")
        e = expected.get(qid)
        if t == "choice":
            keys = list(q.get("criteria", {}).keys())
            if e not in keys:
                return None  # rótulo fora do domínio: descarta a linha
            out[qid] = _one_hot(len(keys), keys.index(e))
        elif t == "noul":
            p = float(bool(e))
            out[qid] = [1.0 - p, p]
        elif t == "score":
            levels = q.get("criteria") or [str(i) for i in range(4)]
            n = len(levels)
            idx = int(e) if isinstance(e, (int, float)) else levels.index(e)
            if not (0 <= idx < n):
                return None
            out[qid] = _one_hot(n, idx)
        else:
            return None
    # todas as perguntas precisam de target
    if set(out) != set(questions):
        return None
    return out
