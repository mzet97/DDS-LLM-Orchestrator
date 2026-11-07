#!/usr/bin/env python3
"""T912-F3 — Treino na RX 7900 XTX (ROCm). Derivado do t911_train_v2.py (provado).

Mudanças T912:
- perguntas: bloco de 12 (4 eixos + 8 cap__*) => gold de 12 chaves bate com as perguntas
  (corrige o bug E1 da T911: pergunta capabilities sem gold era pulada)
- checkpoints POR ÉPOCA em NVMe (~/.cache/tese-laya/t912_train/arm_{C0,C1}/ep{k}/)
  com SHA-256 registrados no train_report.json
- braços: C0 = Qwen3.5-2B (primário), C1 = Qwen3.5-0.8B (baseline)
- arg --mb (default 16) p/ tuning de VRAM 24GB

Uso: python train_t912.py C0 [--mb 16] [--epochs 3]
"""
import hashlib
import json
import os
import sys
import time

sys.path.insert(0, "/var/mnt/HD1TB/tese/src/rust/workers/laya-worker/src")
from laya_worker.questions import QUESTIONS_V1, CAPABILITY_CATALOG

DATA = "/var/mnt/HD1TB/tese/src/laya_t900_handoff/t912/datasets"
WORK = os.path.expanduser("~/.cache/tese-laya/t912_train")
SEED = 909
CAPS = list(CAPABILITY_CATALOG)
Q12 = {k: QUESTIONS_V1[k] for k in ("qos_profile", "needs_planning", "complexity", "urgency")}
for c in CAPS:
    Q12[f"cap__{c}"] = {"type": "noul",
                        "instructions": f"Does this request require the capability '{c}'?"}

ENCODER = {"C0": "unsloth/Qwen3.5-2B", "C1": "unsloth/Qwen3.5-0.8B"}


def log(m):
    print(f"[{time.strftime('%H:%M:%S')}] {m}", flush=True)


def sha(p):
    h = hashlib.sha256()
    with open(p, "rb") as f:
        for c in iter(lambda: f.read(1 << 20), b""):
            h.update(c)
    return h.hexdigest()


def main():
    ARM = sys.argv[1] if len(sys.argv) > 1 else "C0"
    mb = 16
    epochs = 3
    for i, a in enumerate(sys.argv[2:], 2):
        if a == "--mb":
            mb = int(sys.argv[i + 1])
        elif a == "--epochs":
            epochs = int(sys.argv[i + 1])
    out_dir = f"{WORK}/arm_{ARM}"
    os.makedirs(out_dir, exist_ok=True)
    log(f"ARM={ARM} encoder={ENCODER[ARM]} mb={mb} epochs={epochs}")

    from unsloth import FastDecisionModel, DecisionTrainer, is_bfloat16_supported
    from datasets import Dataset
    from transformers import TrainingArguments

    model, tokenizer = FastDecisionModel.from_pretrained(
        model_name=ENCODER[ARM], max_seq_length=2048, load_in_4bit=False)
    model = FastDecisionModel.get_peft_model(
        model, r=16, lora_alpha=16, lora_dropout=0,
        use_gradient_checkpointing="unsloth", random_state=SEED)

    # dataset: rows {state, questions, gold} — substitui questions pelo bloco canônico de 12
    rows = [json.loads(l) for l in open(f"{DATA}/train_t912.jsonl") if l.strip()]
    for r in rows:
        r["questions"] = Q12
    log(f"train rows: {len(rows)}")

    items, report = FastDecisionModel.build_dataset(Dataset.from_list(rows), tokenizer, model)
    skipped = report.get("skipped", 0) if isinstance(report, dict) else "?"
    log(f"build_dataset: {len(items)} itens (skipped={skipped}) — clef 1 item/linha")
    assert len(items) == len(rows), "clef: itens != linhas"

    targs = TrainingArguments(
        per_device_train_batch_size=mb, gradient_accumulation_steps=1,
        warmup_steps=20, num_train_epochs=epochs,
        learning_rate=2e-4, lr_scheduler_type="cosine", weight_decay=0.01,
        bf16=is_bfloat16_supported(), eval_strategy="no",
        logging_steps=25, output_dir=f"{out_dir}/tmp_out", report_to="none",
        seed=SEED, save_strategy="no", dataloader_num_workers=0)

    # checkpoint limpo por época (adapter+joint_head via save_pretrained, não HF ckpt)
    from transformers import TrainerCallback
    steps_per_epoch = max(1, len(items) // mb + (1 if len(items) % mb else 0))

    class EpochSave(TrainerCallback):
        def on_step_end(self, args, state, control, model=None, **kw):
            if state.is_world_process_zero and state.global_step % steps_per_epoch == 0:
                ep = state.global_step // steps_per_epoch
                if 0 < ep < epochs:
                    d = f"{out_dir}/ep{ep}"
                    os.makedirs(d, exist_ok=True)
                    model.save_pretrained(d)
                    log(f"checkpoint ep{ep} salvo -> {d} "
                        f"(sha {sha(d + '/adapter_model.safetensors')[:16]})")
            return control

    trainer = DecisionTrainer(model=model, processing_class=tokenizer,
                              train_dataset=items, args=targs, callbacks=[EpochSave()])

    t0 = time.time()
    log("treino iniciado")
    trainer.train()
    train_s = time.time() - t0
    log(f"treino concluido em {train_s:.0f}s")

    # save final + smoke predict
    model.save_pretrained(out_dir)
    tokenizer.save_pretrained(out_dir)
    m2, tok2 = FastDecisionModel.from_pretrained(model_name=out_dir, max_seq_length=2048)
    FastDecisionModel.for_inference(m2)
    ans = FastDecisionModel.predict(m2, tok2, rows[0]["state"]["text"], Q12)
    assert len(ans) == 12, f"predict {len(ans)} != 12"
    rep = {"arm": ARM, "encoder": ENCODER[ARM], "mb": mb, "epochs": epochs,
           "train_rows": len(rows), "items": len(items), "train_seconds": round(train_s),
           "adapter_sha256": sha(f"{out_dir}/adapter_model.safetensors"),
           "joint_head_sha256": sha(f"{out_dir}/joint_head.safetensors"),
           "predict_smoke_keys": sorted(ans.keys()),
           "predict_sample": {"qos": ans["qos_profile"].get("answer"),
                              "cap__rust_refactor": ans["cap__rust_refactor"].get("noul")},
           "finished": time.strftime("%Y-%m-%dT%H:%M:%S")}
    json.dump(rep, open(f"{out_dir}/train_report.json", "w"), indent=1)
    log(f"TRAIN_COMPLETE {ARM}: adapter={rep['adapter_sha256'][:16]} "
        f"rust_noul={rep['predict_sample']['cap__rust_refactor']}")


if __name__ == "__main__":
    main()
