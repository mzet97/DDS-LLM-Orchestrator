#!/usr/bin/env python3
"""
Stub Python para publicar Tasks via DDS (REQ-101/102).

Uso: python py_stub_pub.py [--count N] [--domain ID]

Publica N Tasks no tópico "Tasks" para teste de interop com Rust.
"""

import argparse
import sys
import time
import uuid

# Adiciona o path do orquestrador Python
import os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "orchestrator"))

from dds_backend.dds_data_space import DDSDataSpace
from orchestrator.models import Task, TaskStatus, TaskPriority, ModelSpecialization


def main():
    parser = argparse.ArgumentParser(description="Publica Tasks via DDS")
    parser.add_argument("--count", type=int, default=10, help="Número de Tasks")
    parser.add_argument("--domain", type=int, default=0, help="DDS Domain ID")
    args = parser.parse_args()

    print(f"[py_stub_pub] Iniciando no domínio {args.domain}, publicando {args.count} Tasks")

    ds = DDSDataSpace(domain_id=args.domain)

    # Aguarda discovery/SEDp casar com os readers (QoS Volatile:
    # amostras escritas antes do match são descartadas).
    time.sleep(2.5)

    now_ns = time.time_ns()

    for i in range(args.count):
        task = Task(
            task_id=f"py-task-{i:04d}",
            client_id="py-client",
            model_required=ModelSpecialization.TEXT,
            model_name="qwen3.5-0.8b",
            messages_json='[{"role":"user","content":"Hello from Python!"}]',
            temperature=0.7,
            max_tokens=256,
            stream=False,
            status=TaskStatus.PENDING,
            priority=TaskPriority.NORMAL,
            created_at_ns=now_ns + i,
            deadline_ns=now_ns + 60_000_000_000,  # +60s
        )

        ds.write_task(task)
        print(f"[py_stub_pub] Task {i+1} publicada: task_id={task.task_id}")

    print(f"[py_stub_pub] Concluído: {args.count} Tasks publicadas")
    time.sleep(2.0)  # linger: garante entrega antes de destruir o writer
    ds.shutdown()


if __name__ == "__main__":
    main()
