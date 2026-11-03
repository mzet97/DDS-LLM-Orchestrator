#!/usr/bin/env python3
"""
Stub Python para publicar TaskOutput com seq_num crescente (REQ-105).

Uso: python py_stub_pub_stream.py [--count N] [--domain ID]

Publica N TaskOutput no tópico "TaskOutput" para teste de interop streaming com Rust.
"""

import argparse
import sys
import time

# Adiciona o path do orquestrador Python
import os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "orchestrator"))

from dds_backend.dds_data_space import DDSDataSpace
from orchestrator.models import TaskOutput, FinishReason


def main():
    parser = argparse.ArgumentParser(description="Publica TaskOutput streaming via DDS")
    parser.add_argument("--count", type=int, default=1000, help="Número de chunks")
    parser.add_argument("--domain", type=int, default=0, help="DDS Domain ID")
    args = parser.parse_args()

    print(f"[py_stub_pub_stream] Iniciando no domínio {args.domain}, publicando {args.count} chunks")

    ds = DDSDataSpace(domain_id=args.domain)

    # Aguarda discovery/SEDp casar com os readers (QoS Volatile:
    # amostras escritas antes do match são descartadas).
    time.sleep(2.5)

    now_ns = time.time_ns()
    task_id = f"py-stream-{now_ns}"

    for i in range(args.count):
        is_final = i == args.count - 1
        output = TaskOutput(
            task_id=task_id,
            seq_num=i,
            content=f"chunk-{i:04d}",
            is_final=is_final,
            finish_reason=FinishReason.COMPLETION if is_final else FinishReason.NONE,
            agent_id="py-agent",
            token_count=1,
            emitted_at_ns=now_ns + i,
        )

        ds.write_output(output)

        if i % 100 == 0:
            print(f"[py_stub_pub_stream] Chunk {i}/{args.count} publicado: seq_num={i}")

    print(f"[py_stub_pub_stream] Concluído: {args.count} chunks publicados, task_id={task_id}")
    time.sleep(2.0)  # linger: garante entrega antes de destruir o writer
    ds.shutdown()


if __name__ == "__main__":
    main()
