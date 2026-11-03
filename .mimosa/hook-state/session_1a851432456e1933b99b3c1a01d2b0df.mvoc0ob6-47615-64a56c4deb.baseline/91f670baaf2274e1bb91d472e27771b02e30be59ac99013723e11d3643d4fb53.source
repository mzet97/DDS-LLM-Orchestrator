#!/usr/bin/env python3
"""
Stub Python para assinar TaskOutput e contar gaps (REQ-105).

Uso: python py_stub_sub_stream.py [--domain ID] [--timeout SEC]

Assina TaskOutput no tópico "TaskOutput" e conta gaps em seq_num.
"""

import argparse
import sys
import time

# Adiciona o path do orquestrador Python
import os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "orchestrator"))

from dds_backend.dds_data_space import DDSDataSpace


def main():
    parser = argparse.ArgumentParser(description="Assina TaskOutput streaming via DDS")
    parser.add_argument("--domain", type=int, default=0, help="DDS Domain ID")
    parser.add_argument("--timeout", type=int, default=30, help="Timeout em segundos")
    args = parser.parse_args()

    print(f"[py_stub_sub_stream] Iniciando no domínio {args.domain}, timeout {args.timeout}s")

    ds = DDSDataSpace(domain_id=args.domain)

    start = time.time()
    received = set()
    total_count = 0
    task_id = None
    got_final = False

    while time.time() - start < args.timeout:
        # Lê outputs do cache
        if task_id:
            outputs = ds.read_outputs(task_id)
        else:
            # Tenta ler de todas as tasks conhecidas
            outputs = []
            for t in ds.all_tasks():
                outputs.extend(ds.read_outputs(t.task_id))

        for output in outputs:
            total_count += 1

            if task_id is None:
                task_id = output.task_id
                print(f"[py_stub_sub_stream] Recebendo chunks de task_id={task_id}")

            if output.task_id == task_id:
                received.add(output.seq_num)

                if output.is_final:
                    got_final = True
                    print(f"[py_stub_sub_stream] Chunk final recebido: seq_num={output.seq_num}")

        if got_final:
            break

        time.sleep(0.001)  # 1ms poll

    # Calcula gaps
    if task_id:
        max_seq = max(received) if received else 0
        expected_count = max_seq + 1
        gaps = [i for i in range(expected_count) if i not in received]

        print(f"\n[py_stub_sub_stream] === Resultado ===")
        print(f"[py_stub_sub_stream] Task ID: {task_id}")
        print(f"[py_stub_sub_stream] Chunks esperados: {expected_count}")
        print(f"[py_stub_sub_stream] Chunks recebidos: {len(received)}")
        print(f"[py_stub_sub_stream] Total de samples: {total_count}")
        print(f"[py_stub_sub_stream] Gaps encontrados: {len(gaps)}")

        if not gaps:
            print(f"[py_stub_sub_stream] ✓ SUCESSO: 0 gaps em {len(received)} chunks!")
        else:
            print(f"[py_stub_sub_stream] ✗ FALHA: {len(gaps)} gaps detectados")
            if len(gaps) <= 20:
                print(f"[py_stub_sub_stream] Gaps: {gaps}")
            else:
                print(f"[py_stub_sub_stream] Primeiros 20 gaps: {gaps[:20]}")
            sys.exit(1)
    else:
        print(f"[py_stub_sub_stream] Nenhum chunk recebido")
        sys.exit(1)

    ds.shutdown()


if __name__ == "__main__":
    main()
