"""Análise (T-907-10 / REQ-907-011): veredictos conhecidos em fixtures sintéticas."""

from __future__ import annotations

import json

from baselines.analysis import (hodges_lehmann_paired, load_totals, percentile,
                                tost_paired)


def _write_jsonl(tmp_path, name, totals, warmup=2, failed=1):
    lines = []
    idx = 0
    for i in range(warmup):
        lines.append(json.dumps({"status": "completed", "warmup": True,
                                 "t_total_s": 9.99, "run_index": idx}))
        idx += 1
    for v in totals:
        lines.append(json.dumps({"status": "completed", "warmup": False,
                                 "t_total_s": v, "run_index": idx}))
        idx += 1
    lines.append(json.dumps({"status": "failed", "warmup": False,
                             "error": "x", "run_index": idx}))
    p = tmp_path / name
    p.write_text("\n".join(lines) + "\n")
    return p


def test_load_totals_exclui_warmup_e_failed(tmp_path):
    p = _write_jsonl(tmp_path, "a.jsonl", [1.0, 1.1, 1.2])
    xs = load_totals(p)
    assert xs == [1.0, 1.1, 1.2]


def test_percentis_conhecidos():
    xs = [float(i) for i in range(1, 101)]  # 1..100
    assert percentile(xs, 0.5) == 50.5
    assert percentile(xs, 0.95) == 95.05
    assert percentile([], 0.5) != percentile([], 0.5)  # NaN


def test_tost_distribuicoes_iguais_sao_equivalentes():
    a = [1.000 + 0.001 * (i % 7) for i in range(40)]
    b = list(a)
    r = tost_paired(a, b, mei=0.020)
    assert r["verdict"] == "equivalente"
    assert abs(r["mean_delta"]) < 1e-9
    assert abs(r["hodges_lehmann"]) < 1e-9


def test_tost_deslocamento_maior_que_mei_nao_equivalente():
    b = [1.000 + 0.001 * (i % 7) for i in range(40)]
    a = [v + 0.100 for v in b]  # +100 ms > MEI 20 ms
    r = tost_paired(a, b, mei=0.020)
    assert r["verdict"] == "NAO-equivalente"
    assert r["ci90_low"] > 0.020 or r["ci90_high"] < -0.020


def test_tost_inconclusivo_com_poucos_dados():
    r = tost_paired([1.0], [1.0], mei=0.020)
    assert r["verdict"] == "inconclusivo"


def test_hodges_lehmann_pareado_basico():
    assert abs(hodges_lehmann_paired([1.0, 2.0, 3.0]) - 2.0) < 1e-9
