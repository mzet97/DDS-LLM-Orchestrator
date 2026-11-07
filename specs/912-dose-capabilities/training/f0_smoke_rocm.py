#!/usr/bin/env python3
"""T912-F0 — Smoke Unsloth FastDecisionModel na RX 7900 XTX (ROCm gfx1100).

Valida o caminho completo de treino ANTES da campanha:
import → from_pretrained(Qwen3.5-2B) → get_peft_model(r16) → build_dataset com
DICT DE 12 PERGUNTAS (4 eixos + 8 cap__*) → forward/backward/step → save → reload →
predict 12 perguntas. Grava t912/infra/SMOKE_ROCM.json.

Regra do autor: se falhar, diagnosticar e documentar ANTES de qualquer mudança de plataforma.
"""
import json, os, sys, time, subprocess, traceback

sys.path.insert(0, "/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/src")
OUT = {"phase": "F0-smoke-rocm", "started": time.strftime("%Y-%m-%dT%H:%M:%S")}
CAPS = ["rust_refactor", "python_refactor", "unit_testing", "security_audit",
        "code_review", "research", "summarization", "translation"]


def log(m):
    print(f"[smoke] {m}", flush=True)
    OUT.setdefault("log", []).append(f"{time.strftime('%H:%M:%S')} {m}")


def gpu_state():
    try:
        r = subprocess.run(["rocm-smi", "--showuse", "--showtemp", "--showmeminfo", "vram",
                            "--json"], capture_output=True, text=True, timeout=10)
        return json.loads(r.stdout) if r.stdout.strip() else {"raw": r.stderr[:200]}
    except Exception as e:
        return {"error": str(e)[:200]}


try:
    import torch
    OUT["torch"] = torch.__version__
    OUT["hip"] = getattr(torch.version, "hip", None)
    OUT["cuda_available"] = torch.cuda.is_available()
    OUT["device_name"] = torch.cuda.get_device_name(0) if OUT["cuda_available"] else None
    OUT["gcn_arch"] = torch.cuda.get_device_capability(0) if OUT["cuda_available"] else None
    log(f"torch {torch.__version__} hip {OUT['hip']} avail {OUT['cuda_available']} ({OUT['device_name']})")

    from unsloth import FastDecisionModel
    import unsloth
    OUT["unsloth"] = getattr(unsloth, "__version__", "?")
    log(f"unsloth importado ({OUT['unsloth']})")

    from laya_worker.questions import QUESTIONS_V1, CAPABILITY_CATALOG
    # bloco 12 perguntas (receita canônica do evaluator_t909/adapter worker)
    Q12 = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
    for c in CAPABILITY_CATALOG:
        Q12[f"cap__{c}"] = {"type": "noul",
                            "instructions": f"Does this request require the capability '{c}'?"}
    OUT["n_questions"] = len(Q12)
    assert len(Q12) == 12 and CAPABILITY_CATALOG == CAPS

    MODEL = sys.argv[1] if len(sys.argv) > 1 else "unsloth/Qwen3.5-2B"
    OUT["model"] = MODEL
    log(f"carregando {MODEL} (load_in_4bit=False)")
    t0 = time.time()
    model, tokenizer = FastDecisionModel.from_pretrained(
        model_name=MODEL, max_seq_length=2048, load_in_4bit=False)
    OUT["load_s"] = round(time.time() - t0, 1)
    model = FastDecisionModel.get_peft_model(
        model, r=16, lora_alpha=16, lora_dropout=0,
        use_gradient_checkpointing="unsloth", random_state=909)
    log(f"peft ok em {OUT['load_s']}s")

    # dataset mínimo com 12 perguntas E gold de 12 chaves
    rows = [{
        "state": {"text": "Refactor the Rust module src/parser.rs to remove unsafe blocks and add unit tests for parse_edge_cases. Critical production fix, the service is down now."},
        "questions": Q12,
        "gold": {"qos_profile": "QI_CRITICAL", "needs_planning": True, "complexity": 2,
                 "urgency": 2, "cap__rust_refactor": True, "cap__python_refactor": False,
                 "cap__unit_testing": True, "cap__security_audit": False,
                 "cap__code_review": False, "cap__research": False,
                 "cap__summarization": False, "cap__translation": False},
    }, {
        "state": {"text": "Translate this weekly summary email to Portuguese, no rush."},
        "questions": Q12,
        "gold": {"qos_profile": "QI_LOW_COST", "needs_planning": False, "complexity": 0,
                 "urgency": 0, "cap__rust_refactor": False, "cap__python_refactor": False,
                 "cap__unit_testing": False, "cap__security_audit": False,
                 "cap__code_review": False, "cap__research": False,
                 "cap__summarization": False, "cap__translation": True},
    }]
    from datasets import Dataset
    t0 = time.time()
    items, report = FastDecisionModel.build_dataset(Dataset.from_list(rows), tokenizer, model)
    OUT["build_dataset_s"] = round(time.time() - t0, 1)
    OUT["build_report"] = {k: (v if not isinstance(v, (list, dict)) else str(v)[:200])
                           for k, v in (report.items() if isinstance(report, dict) else [])}
    OUT["n_items"] = len(items)
    # Qwen3.5 = layout CLEF: 1 item por LINHA com TODAS as perguntas no mesmo prompt
    # (targets/labels/qtypes em listas por pergunta). A asserção relevante é que NENHUMA
    # pergunta foi pulada por falta de gold (bug E1 da T911) e que o item carrega 12 targets.
    layout = getattr(model, "is_clef", None) or (getattr(model, "decision_config", {}) or {}).get("layout")
    OUT["layout"] = str(layout)
    assert len(items) == len(rows), f"clef: esperado {len(rows)} itens (1/linha), veio {len(items)}"
    it0 = items[0]
    OUT["item0_keys"] = sorted(it0.keys()) if isinstance(it0, dict) else str(type(it0))
    n_targets = None
    for k in ("targets", "labels", "qtypes", "answers"):
        v = it0.get(k) if isinstance(it0, dict) else None
        if isinstance(v, (list, tuple)):
            n_targets = n_targets or len(v)
    OUT["item0_n_targets"] = n_targets
    if n_targets is not None:
        assert n_targets == 12, f"item clef com {n_targets} targets, esperado 12 (bug E1 se <12)"
    log(f"build_dataset: {len(items)} itens clef (1/linha, {n_targets} targets/perguntas) em {OUT['build_dataset_s']}s")

    # forward/backward/step (micro-batch 4)
    from transformers import TrainingArguments
    from unsloth import DecisionTrainer, is_bfloat16_supported
    targs = TrainingArguments(
        per_device_train_batch_size=4, gradient_accumulation_steps=1,
        max_steps=6, learning_rate=2e-4, warmup_steps=0,
        bf16=is_bfloat16_supported(), logging_steps=1, output_dir="/tmp/t912_smoke_out",
        report_to="none", seed=909, save_strategy="no")
    trainer = DecisionTrainer(model=model, processing_class=tokenizer,
                              train_dataset=items, args=targs)
    t0 = time.time()
    trainer.train()
    OUT["train_6steps_s"] = round(time.time() - t0, 1)
    OUT["gpu_after_train"] = gpu_state()
    log(f"6 steps de treino em {OUT['train_6steps_s']}s")

    # save → reload → predict
    DST = os.path.expanduser("~/.cache/tese-laya/t912_smoke/arm_smoke")
    model.save_pretrained(DST)
    tokenizer.save_pretrained(DST)
    import hashlib
    OUT["adapter_sha256_16"] = hashlib.sha256(
        open(DST + "/adapter_model.safetensors", "rb").read()).hexdigest()[:16]
    m2, tok2 = FastDecisionModel.from_pretrained(model_name=DST, max_seq_length=2048)
    FastDecisionModel.for_inference(m2)
    ans = FastDecisionModel.predict(m2, tok2, rows[0]["state"]["text"], Q12)
    OUT["predict_keys"] = sorted(ans.keys())
    assert len(ans) == 12, f"predict retornou {len(ans)} respostas, esperado 12"
    qos_ans = ans["qos_profile"].get("choice") or ans["qos_profile"].get("answer")
    OUT["predict_sample"] = {"qos": qos_ans,
                             "cap__rust_refactor": ans["cap__rust_refactor"].get("noul"),
                             "cx": ans["complexity"].get("answer")}
    log(f"predict 12 perguntas OK: qos={qos_ans} rust_noul={OUT['predict_sample']['cap__rust_refactor']}")

    OUT["status"] = "PASS"
    OUT["gpu_final"] = gpu_state()
    OUT["finished"] = time.strftime("%Y-%m-%dT%H:%M:%S")
except Exception as e:
    OUT["status"] = "FAIL"
    OUT["error"] = f"{type(e).__name__}: {e}"
    OUT["traceback"] = traceback.format_exc()[-3000:]
    OUT["gpu_on_fail"] = gpu_state()
    log(f"FALHOU: {OUT['error']}")

dst = "/var/mnt/HD1TB/tese/src/laya_t900_handoff/t912/infra/SMOKE_ROCM.json"
json.dump(OUT, open(dst, "w"), indent=1, default=str)
print(f"[smoke] {OUT['status']} -> {dst}")
sys.exit(0 if OUT["status"] == "PASS" else 1)
