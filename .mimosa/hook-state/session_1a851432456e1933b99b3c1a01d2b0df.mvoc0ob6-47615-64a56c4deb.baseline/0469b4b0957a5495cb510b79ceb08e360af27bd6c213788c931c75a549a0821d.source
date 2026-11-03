#!/usr/bin/env python3
"""
Benchmark RTT Rust-vs-Python (REQ-104).

Uso: python benchmark_rtt.py [--domain ID] [--samples N]

Mede latência round-trip publicando Task → recebendo TaskOutput ecoado.
Metodologia:
1. Publica Task no tópico Tasks
2. Espera TaskOutput ecoado no tópico TaskOutput
3. Mede tempo entre publicação e recebimento
4. Repete N vezes e calcula p50/p95/p99
"""

import argparse
import statistics
import sys
import time

import os
sys.path.insert(0, os.path.join(os.path.dirname(os.path.abspath(__file__)), "..", "..", "..", "..", "orchestrator"))

from dds_backend.dds_data_space import DDSDataSpace
from orchestrator.models import (
    Task, TaskOutput, TaskStatus, TaskPriority, ModelSpecialization, FinishReason
)


def run_benchmark(domain_id: int, num_samples: int, warmup: int = 100):
    """Executa benchmark de latência RTT."""
    print(f"[benchmark] Iniciando: domain={domain_id}, samples={num_samples}, warmup={warmup}")

    ds = DDSDataSpace(domain_id=domain_id)

    # Warmup
    print(f"[benchmark] Warmup: {warmup} amostras...")
    for i in range(warmup):
        task = Task(
            task_id=f"warmup-{i}",
            client_id="benchmark",
            model_required=ModelSpecialization.TEXT,
            model_name="qwen3.5-0.8b",
            messages_json='[{"role":"user","content":"warmup"}]',
            temperature=0.7,
            max_tokens=10,
            stream=False,
            status=TaskStatus.PENDING,
            priority=TaskPriority.NORMAL,
            created_at_ns=time.time_ns(),
            deadline_ns=time.time_ns() + 60_000_000_000,
        )
        ds.write_task(task)
        time.sleep(0.001)  # 1ms entre samples

    # Coleta de latências
    latencies_ns = []
    print(f"[benchmark] Coletando {num_samples} amostras...")

    for i in range(num_samples):
        task_id = f"bench-{i}-{time.time_ns()}"
        now_ns = time.time_ns()

        task = Task(
            task_id=task_id,
            client_id="benchmark",
            model_required=ModelSpecialization.TEXT,
            model_name="qwen3.5-0.8b",
            messages_json='[{"role":"user","content":"benchmark"}]',
            temperature=0.7,
            max_tokens=10,
            stream=False,
            status=TaskStatus.PENDING,
            priority=TaskPriority.NORMAL,
            created_at_ns=now_ns,
            deadline_ns=now_ns + 60_000_000_000,
        )

        # Publica task
        ds.write_task(task)

        # Espera output ecoado (timeout 5s)
        start = time.monotonic()
        timeout = 5.0
        received = False

        while time.monotonic() - start < timeout:
            outputs = ds.read_outputs(task_id)
            if outputs:
                end_ns = time.time_ns()
                latency_ns = end_ns - now_ns
                latencies_ns.append(latency_ns)
                received = True
                break
            time.sleep(0.0001)  # 0.1ms poll

        if not received:
            print(f"[benchmark] WARNING: timeout na amostra {i}")

        if i % 100 == 0 and i > 0:
            print(f"[benchmark] {i}/{num_samples} amostras coletadas")

    # Estatísticas
    if not latencies_ns:
        print("[benchmark] ERRO: nenhuma amostra válida")
        return None

    latencies_ms = [l / 1_000_000 for l in latencies_ns]
    latencies_ms.sort()

    stats = {
        "samples": len(latencies_ms),
        "min_ms": latencies_ms[0],
        "max_ms": latencies_ms[-1],
        "mean_ms": statistics.mean(latencies_ms),
        "median_ms": statistics.median(latencies_ms),
        "p95_ms": latencies_ms[int(len(latencies_ms) * 0.95)],
        "p99_ms": latencies_ms[int(len(latencies_ms) * 0.99)],
        "stdev_ms": statistics.stdev(latencies_ms) if len(latencies_ms) > 1 else 0,
    }

    print(f"\n[benchmark] === Resultado Python ===")
    print(f"[benchmark] Amostras válidas: {stats['samples']}")
    print(f"[benchmark] Mínimo: {stats['min_ms']:.3f} ms")
    print(f"[benchmark] Média: {stats['mean_ms']:.3f} ms")
    print(f"[benchmark] Mediana (p50): {stats['median_ms']:.3f} ms")
    print(f"[benchmark] p95: {stats['p95_ms']:.3f} ms")
    print(f"[benchmark] p99: {stats['p99_ms']:.3f} ms")
    print(f"[benchmark] Máximo: {stats['max_ms']:.3f} ms")
    print(f"[benchmark] Desvio padrão: {stats['stdev_ms']:.3f} ms")

    ds.shutdown()
    return stats


def main():
    parser = argparse.ArgumentParser(description="Benchmark RTT Python")
    parser.add_argument("--domain", type=int, default=0, help="DDS Domain ID")
    parser.add_argument("--samples", type=int, default=10000, help="Número de amostras")
    parser.add_argument("--warmup", type=int, default=100, help="Amostras de warmup")
    args = parser.parse_args()

    stats = run_benchmark(args.domain, args.samples, args.warmup)
    if stats:
        # Salva resultados em JSON
        import json
        with open("benchmark_python_results.json", "w") as f:
            json.dump(stats, f, indent=2)
        print(f"\n[benchmark] Resultados salvos em benchmark_python_results.json")


if __name__ == "__main__":
    main()
