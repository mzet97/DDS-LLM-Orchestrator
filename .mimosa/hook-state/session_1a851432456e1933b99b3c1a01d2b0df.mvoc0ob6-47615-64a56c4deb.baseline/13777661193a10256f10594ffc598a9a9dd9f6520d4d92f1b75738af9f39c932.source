#!/usr/bin/env python3
"""
Agente Python stub para o teste A/B de coexistência (T-207).

Espelha a lógica de claim do agente Rust:
  PENDING → claim (ASSIGNED c/ meu id) → readback (250 ms) → executa (DONE + output).

Uso: python py_agent_stub.py --agent-id ID --domain D --seconds N
Saída: linhas "[py-agent] PROCESSED <task_id>" e "TOTAL_PROCESSED=N".
"""

import argparse
import os
import sys
import time

sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "orchestrator"))

from dds_backend.dds_data_space import DDSDataSpace
from orchestrator.models import TaskOutput, TaskStatus, FinishReason


def mesh_state(ds, task_id):
    """Lê o estado ARBITRADO do mesh (RHC do reader), não o cache do DDSDataSpace.

    Com Exclusive Ownership + strengths iguais, o RHC mantém a versão vencedora
    (empate → menor GUID, determinístico nos dois lados). O cache local do
    DDSDataSpace tem write-through e overwrite por chegada — o 2º a clamar sempre
    se auto-confirmaria (execução dupla)."""
    latest = None
    try:
        for s in ds.reader_tasks.read(N=256):
            if s.task_id == task_id:
                latest = s
    except Exception:
        return None
    return latest


def main():
    parser = argparse.ArgumentParser()
    parser.add_argument("--agent-id", required=True)
    parser.add_argument("--domain", type=int, default=0)
    parser.add_argument("--seconds", type=int, default=90)
    args = parser.parse_args()

    print(f"[py-agent] {args.agent_id} iniciando no domínio {args.domain}")
    ds = DDSDataSpace(domain_id=args.domain)
    claimed = set()
    processed = []
    start = time.time()

    while time.time() - start < args.seconds:
        for task in ds.all_tasks():
            if task.task_id in claimed or task.status != TaskStatus.PENDING:
                continue
            claimed.add(task.task_id)
            task.assigned_agent = args.agent_id
            task.status = TaskStatus.ASSIGNED
            task.assigned_at_ns = time.time_ns()
            ds.write_task(task)

            time.sleep(0.25)  # janela de readback (arbitragem)
            cur = mesh_state(ds, task.task_id)
            if cur and cur.assigned_agent == args.agent_id and cur.status == TaskStatus.ASSIGNED:
                done = cur
                done.status = TaskStatus.DONE
                done.completed_at_ns = time.time_ns()
                ds.write_task(done)
                ds.write_output(TaskOutput(
                    task_id=cur.task_id, seq_num=0, content="done", is_final=True,
                    finish_reason=FinishReason.COMPLETION, agent_id=args.agent_id,
                    token_count=1, emitted_at_ns=time.time_ns(),
                ))
                processed.append(task.task_id)
                print(f"[py-agent] PROCESSED {task.task_id}", flush=True)
        time.sleep(0.005)

    print(f"[py-agent] TOTAL_PROCESSED={len(processed)}", flush=True)
    ds.shutdown()


if __name__ == "__main__":
    main()
