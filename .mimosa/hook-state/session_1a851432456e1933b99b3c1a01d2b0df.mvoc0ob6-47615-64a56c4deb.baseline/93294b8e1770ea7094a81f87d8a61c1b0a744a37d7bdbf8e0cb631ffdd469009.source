#!/usr/bin/env python3
"""
Echo stub Python para o benchmark RTT (REQ-104).

Assina Tasks e devolve um TaskOutput (seq_num=0, is_final=True) com o mesmo
task_id — fecha o round-trip medido pelo benchmark_rtt.py.

Uso: python py_echo.py [--domain ID] [--seconds N]
"""

import argparse
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "orchestrator"))

from dds_backend.dds_data_space import DDSDataSpace
from orchestrator.models import TaskOutput, FinishReason


def main():
    parser = argparse.ArgumentParser(description="Echo Task->TaskOutput via DDS")
    parser.add_argument("--domain", type=int, default=0, help="DDS Domain ID")
    parser.add_argument("--seconds", type=int, default=600, help="Tempo ativo")
    args = parser.parse_args()

    print(f"[py_echo] Iniciando no domínio {args.domain} por {args.seconds}s")

    ds = DDSDataSpace(domain_id=args.domain)
    echoed = set()
    start = time.time()

    while time.time() - start < args.seconds:
        for task in ds.all_tasks():
            if task.task_id in echoed:
                continue
            if not task.task_id.startswith(("bench-", "warmup-")):
                continue
            echoed.add(task.task_id)
            ds.write_output(TaskOutput(
                task_id=task.task_id,
                seq_num=0,
                content="echo",
                is_final=True,
                finish_reason=FinishReason.COMPLETION,
                agent_id="py-echo",
                token_count=1,
                emitted_at_ns=time.time_ns(),
            ))
        time.sleep(0.0002)  # 0.2ms poll

    print(f"[py_echo] Concluído: {len(echoed)} tasks ecoadas")
    ds.shutdown()


if __name__ == "__main__":
    main()
