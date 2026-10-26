"""Backend determinístico Chat-Completions-compatível (Prompt §11). Só stdlib.

Protocolo: o cliente envia `messages` (conteúdo puro do prompt) mais um objeto
opcional `bench` fora da banda do prompt: {"stage": "A|B|C", "expects": [hashes]}.
A resposta carrega fixture verificável: stage + sha256 do conteúdo normalizado.
O validador (§7) confere dependências (B recebeu A, C recebeu A+B) pelos hashes.
"""

from __future__ import annotations

import hashlib
import json
import threading
import time
from http.server import BaseHTTPRequestHandler, ThreadingHTTPServer


def normalize(messages: list[dict]) -> str:
    parts = []
    for m in messages:
        role = str(m.get("role", ""))
        content = m.get("content", "")
        if isinstance(content, list):
            content = " ".join(p.get("text", "") for p in content if isinstance(p, dict))
        parts.append(f"{role}:{content}")
    return "\n".join(parts)


def content_hash(messages: list[dict]) -> str:
    return hashlib.sha256(normalize(messages).encode()).hexdigest()[:16]


class State:
    def __init__(self, delay_ms: int = 0, capacity: int = 16):
        self.delay_ms = delay_ms
        self.sem = threading.Semaphore(capacity)
        self.capacity = capacity
        self.lock = threading.Lock()
        self.requests: list[dict] = []
        self.seq = 0


def make_handler(state: State):
    class H(BaseHTTPRequestHandler):
        def log_message(self, *a):
            pass

        def _send(self, code: int, obj: dict, stream: bool = False):
            body = json.dumps(obj).encode()
            self.send_response(code)
            self.send_header("Content-Type", "application/json")
            self.send_header("Content-Length", str(len(body)))
            self.end_headers()
            if not stream:
                self.wfile.write(body)

        def do_POST(self):
            if self.path not in ("/v1/chat/completions", "/chat/completions"):
                return self._send(404, {"error": "unknown path"})
            try:
                length = int(self.headers.get("Content-Length", 0))
                req = json.loads(self.rfile.read(length) or b"{}")
            except (ValueError, OSError):
                return self._send(400, {"error": "invalid json"})
            messages = req.get("messages")
            if not isinstance(messages, list) or not messages:
                return self._send(400, {"error": "messages must be a non-empty list"})
            bench = req.get("bench", {}) if isinstance(req.get("bench"), dict) else {}
            stage = str(bench.get("stage", "?"))
            arrival = time.monotonic()
            if not state.sem.acquire(timeout=30):
                return self._send(429, {"error": "backend saturated"})
            try:
                started = time.monotonic()
                if state.delay_ms:
                    time.sleep(state.delay_ms / 1000.0)
                h = content_hash(messages)
                content = f"[fixture stage={stage} h={h} n={len(messages)}]"
                with state.lock:
                    state.seq += 1
                    rec = {
                        "seq": state.seq,
                        "stage": stage,
                        "hash": h,
                        "resp": content,
                        "expects": list(bench.get("expects", [])),
                        "arrival_ns": int(arrival * 1e9),
                        "started_ns": int(started * 1e9),
                        "finished_ns": int(time.monotonic() * 1e9),
                        "delay_ms": state.delay_ms,
                    }
                    state.requests.append(rec)
                if req.get("stream"):
                    return self._sse(content, rec)
                return self._send(200, {
                    "id": f"bench-{rec['seq']}",
                    "object": "chat.completion",
                    "choices": [{"index": 0, "message": {"role": "assistant", "content": content},
                                 "finish_reason": "stop"}],
                    "usage": {"prompt_tokens": 10, "completion_tokens": 5, "total_tokens": 15},
                })
            finally:
                state.sem.release()

        def _sse(self, content: str, rec: dict):
            self.send_response(200)
            self.send_header("Content-Type", "text/event-stream")
            self.end_headers()
            for i, half in enumerate([content[: len(content) // 2], content[len(content) // 2:]]):
                chunk = {"id": f"bench-{rec['seq']}", "object": "chat.completion.chunk",
                         "choices": [{"index": 0, "delta": {"content": half}, "finish_reason": None}]}
                data = f"data: {json.dumps(chunk)}\n\n".encode()
                self.wfile.write(data)
            self.wfile.write(b"data: [DONE]\n\n")

    return H


def serve(host: str = "127.0.0.1", port: int = 0, delay_ms: int = 0, capacity: int = 16):
    state = State(delay_ms=delay_ms, capacity=capacity)
    srv = ThreadingHTTPServer((host, port), make_handler(state))
    return srv, state


if __name__ == "__main__":
    import sys
    port = int(sys.argv[1]) if len(sys.argv) > 1 else 8080
    delay = int(sys.argv[2]) if len(sys.argv) > 2 else 0
    srv, _ = serve(port=port, delay_ms=delay)
    print(f"deterministic backend on {srv.server_address}", flush=True)
    srv.serve_forever()
