#!/usr/bin/env python3
"""
Stub Python para assinar Tasks via DDS (REQ-101/102).

Uso: python py_stub_sub.py [--domain ID] [--timeout SEC]

Assina Tasks no tópico "Tasks" para teste de interop com Rust.
"""

import argparse
import sys
import time

# Adiciona o path do orquestrador Python
import os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "orchestrator"))

from dds_backend.dds_data_space import DDSDataSpace


def main():
    parser = argparse.ArgumentParser(description="Assina Tasks via DDS")
    parser.add_argument("--domain", type=int, default=0, help="DDS Domain ID")
    parser.add_argument("--timeout", type=int, default=30, help="Timeout em segundos")
    args = parser.parse_args()

    print(f"[py_stub_sub] Iniciando no domínio {args.domain}, timeout {args.timeout}s")

    ds = DDSDataSpace(domain_id=args.domain)

    start = time.time()
    count = 0

    while time.time() - start < args.timeout:
        tasks = ds.all_tasks()

        for task in tasks:
            if task.task_id.startswith(("spike-task-", "rust-task-")):
                count += 1
                print(
                    f"[py_stub_sub] Task #{count} recebida: "
                    f"task_id={task.task_id}, client_id={task.client_id}, "
                    f"status={task.status.name}, model={task.model_name}"
                )

                # Afirma campos obrigatórios
                assert task.task_id, "task_id não pode ser vazio"
                assert task.created_at_ns > 0, "created_at_ns deve ser > 0"

                print(f"[py_stub_sub] ✓ Campos validados com sucesso")

        time.sleep(0.01)  # 10ms poll

    print(f"[py_stub_sub] Concluído: {count} Tasks recebidas")
    ds.shutdown()


if __name__ == "__main__":
    main()
