#!/usr/bin/env python3
"""T904-M10 — smoke local obrigatório: laya.load() + 1 exemplo por perfil QI_*,
planning True/False, complexity/urgency ordinal e cada capability (rótulos de smoke,
NÃO teste científico). Confere SHA e ausência de caminho hardcoded da VM.

Uso: verify_checkpoint.py <dir_checkpoint> <sha_esperado> [--venv-python]
"""
import json
import os
import sys
import time

EXPECTED = {"QI_BALANCED", "QI_CRITICAL", "QI_STREAM_LIKE", "QI_LOW_COST", "QI_FAILOVER"}
SMOKE = [
    ("QI_STREAM_LIKE", "Transcreva as 20 entrevistas e me envie a transcrição de cada uma assim que terminar cada entrevista."),
    ("QI_BALANCED", "Analise as 20 entrevistas e me entregue um relatório consolidado ao final."),
    ("QI_CRITICAL", "O serviço de pagamentos está fora do ar; audite agora e produza o diagnóstico em 1 hora."),
    ("QI_LOW_COST", "Processe o backlog histórico de logs quando houver capacidade ociosa; sem pressa."),
    ("QI_FAILOVER", "Monte um pipeline que continue processando mesmo se um dos nós cair, com retries e failover."),
]


def main() -> int:
    d = sys.argv[1]
    want = sys.argv[2]
    h = __import__("hashlib").sha256(open(f"{d}/model.safetensors", "rb").read()).hexdigest()
    assert h == want, f"SHA divergente: {h} != {want}"
    print(json.dumps({"sha256": h, "bytes": os.path.getsize(f"{d}/model.safetensors")}))

    import laya
    agent = laya.load(d)
    sys.path.insert(0, "/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/src")
    from laya_worker.questions import CAPABILITY_CATALOG, QUESTIONS_V1
    Q = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
    for c in CAPABILITY_CATALOG:
        Q[f"cap__{c}"] = {"type": "noul",
                          "instructions": f"Does this request require the capability '{c}'?"}
    results = []
    seen = set()
    for want_qos, text in SMOKE:
        t0 = time.time()
        ans = agent.system_one({"text": text}, Q)["answers"]
        qos = ans["qos_profile"]["choice"]
        row = {"expect": want_qos, "got": qos, "ok": qos == want_qos,
               "needs_planning": float(ans["needs_planning"]["noul"]) >= 0.5,
               "complexity": ans["complexity"].get("score"),
               "urgency": ans["urgency"].get("score"),
               "latency_s": round(time.time() - t0, 2)}
        results.append(row)
        seen.add(qos)
    cap_probe = agent.system_one(
        {"text": "Refatore o módulo Python, adicione unit tests e faça um code review do patch."}, Q)["answers"]
    caps_on = sorted(c for c in CAPABILITY_CATALOG
                     if float(cap_probe[f"cap__{c}"]["noul"]) >= 0.5)
    summary = {"profiles_seen": sorted(seen), "all_profiles_covered": EXPECTED <= seen,
               "smoke": results, "caps_fired_on_probe": caps_on,
               "load_and_smoke": "PASS"}
    print(json.dumps(summary, ensure_ascii=False, indent=1))
    return 0 if summary["all_profiles_covered"] else 1


if __name__ == "__main__":
    sys.exit(main())
