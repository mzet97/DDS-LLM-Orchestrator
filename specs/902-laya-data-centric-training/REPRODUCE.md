# Reproducao (comandos executados)
1. colab new -s laya900 --gpu T4; pip install laya==0.4.10
2. Gerar splits: scripts descritos em dataset_card.md (train 2076 SHA 9796703e..., sealed db457693...)
3. laya-evals validate <train>; laya-train --data <train> --eval <dev> --dry-run
4. laya-train --data t902_train.jsonl --eval t902_dev.jsonl --out models/arm_A --base ckpt_t900 --device cuda --seed 902 --epochs 4 --loss rlcd
5. laya-train --data t902_train_focal.jsonl ... arm_B (idem); arm_C com --loss soft-ce
6. Avaliar dev e golden antigo via laya.load()+laya.evals.evaluate (nunca --model <path>)
7. Freeze (evidence/FREEZE_arm_B.json) e abertura unica do sealed (evidence/sealed_result.json)
8. cargo test -p workflow-engine -p dds-contract; pytest workers/laya-worker/tests; harness/run_shadow.py --domain 219
