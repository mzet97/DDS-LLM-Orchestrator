"""Planner LLM para os braços 3/4 da campanha M6 (gemma-4-12b local).

D7: planner GENERATIVO separado do Laya. Usa o servidor llama.cpp local
(openai-compatible) — o mesmo modelo oficial do B-BENCH (UD-Q4_K_XL,
sha256 90fd944d…). Saída: dicionário no formato do `PlanCandidate` Rust
(validado pelo bin `dag-validate`).

Nota de medição: chamamos o servidor HTTP diretamente (como o piloto
B-BENCH); o hop DDS do llm-gateway não entra nesta latência — anotado como
ameaça à validade no manifest.
"""

from __future__ import annotations

import json
import re
import urllib.request

from laya_worker.questions import CAPABILITY_CATALOG

import os
SERVER = os.environ.get("LLM_PLANNER_SERVER", "http://127.0.0.1:8901")

PLANNER_SYSTEM = (
    "You are a task planner. Decompose the user request into 1 to 4 subtasks. "
    "Reply ONLY with a JSON array; each element: "
    '{"node_id": "n1", "dependencies": [], "required_capabilities": ["..."], '
    '"prompt": "..."}. Allowed capabilities: '
    + ", ".join(CAPABILITY_CATALOG)
    + ". Use an empty dependency list for the first task and node ids of "
    "earlier tasks for later ones. Single-step requests need exactly one node."
)


def plan_nodes(prompt: str, max_nodes: int = 4, timeout_s: float = 120.0) -> tuple[list[dict], dict]:
    """Retorna (nodes, meta) — meta com latência/tokens da chamada."""
    import time

    body = json.dumps(
        {
            "messages": [
                {"role": "system", "content": PLANNER_SYSTEM},
                {"role": "user", "content": prompt},
            ],
            "max_tokens": 2048,
            "temperature": 0.0,
        }
    ).encode()
    req = urllib.request.Request(
        f"{SERVER}/v1/chat/completions", data=body,
        headers={"Content-Type": "application/json"},
    )
    t0 = time.perf_counter()
    with urllib.request.urlopen(req, timeout=timeout_s) as resp:
        data = json.load(resp)
    dt_ms = (time.perf_counter() - t0) * 1000
    usage = data.get("usage", {})
    msg = (data.get("choices") or [{}])[0].get("message", {})
    content = msg.get("content") or ""
    nodes = _extract_nodes(content, max_nodes)
    if not nodes:
        # gemma-4 pensa muito: o array às vezes só existe no reasoning.
        nodes = _extract_nodes(msg.get("reasoning_content") or "", max_nodes)
        meta_note = "extracted_from_reasoning"
    else:
        meta_note = "content"

    meta = {
        "latency_ms": round(dt_ms),
        "source": meta_note,
        "completion_tokens": usage.get("completion_tokens", 0),
        "prompt_tokens": usage.get("prompt_tokens", 0),
        "raw_len": len(content),
    }
    return nodes, meta


def _extract_nodes(content: str, max_nodes: int) -> list[dict]:
    m = re.search(r"\[.*\]", content, re.S)
    if not m:
        return []
    try:
        arr = json.loads(m.group(0))
    except json.JSONDecodeError:
        return []
    nodes = []
    for i, item in enumerate(arr[:max_nodes]):
        if not isinstance(item, dict):
            continue
        nodes.append(
            {
                "node_id": str(item.get("node_id") or f"n{i+1}"),
                "dependencies": [str(d) for d in item.get("dependencies", [])],
                "required_capabilities": [
                    str(c) for c in item.get("required_capabilities", [])
                ],
                "prompt": str(item.get("prompt", ""))[:512],
            }
        )
    return nodes


def to_candidate(nodes: list[dict], planner_tokens: int) -> dict:
    """Converte a saída no formato do PlanCandidate Rust (dag-validate)."""
    return {
        "planner_id": "llm-gemma4-12b",
        "planner_revision": "b11671+UD-Q4_K_XL",
        "planner_tokens": planner_tokens,
        "nodes": [
            {
                "node_id": n["node_id"],
                "kind": "inference",
                "required_capabilities": n["required_capabilities"],
                "model_id": "",
                "target_agent_id": "",
                "prompt_template": n["prompt"],
                "dependencies": n["dependencies"],
                "output_names": ["out"],
                "inputs": [],
                "max_attempts": 1,
                "max_tokens": 256,
                "max_output_bytes": 65536,
                "deadline_ns": 0,
                "priority": 5,
                "side_effect_key": "",
            }
            for n in nodes
        ],
    }
