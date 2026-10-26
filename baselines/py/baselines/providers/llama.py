"""Cliente LLM real OpenAI-compatível (llama-server local) — REQ-907-008.

Transporte puro, sem orquestração. Não envia o campo `bench` (exclusivo do backend
determinístico). Sampling fixado por request: temperature/seed/max_tokens.
"""

from __future__ import annotations

import json
import time
from typing import AsyncIterator

import httpx


class LlamaError(Exception):
    """Base dos erros tipados do cliente LLM real."""


class LlamaConnectionError(LlamaError):
    pass


class LlamaTimeoutError(LlamaError):
    pass


class LlamaServerError(LlamaError):
    def __init__(self, status: int, body: str):
        self.status = status
        self.body = body[:512]
        super().__init__(f"llama-server HTTP {status}: {self.body}")


class LlamaMalformedError(LlamaError):
    pass


class LlamaClient:
    def __init__(self, base_url: str, model: str, timeout_s: float = 180.0,
                 temperature: float = 0.0, seed: int | None = None,
                 max_tokens: int = 512, transport: httpx.AsyncBaseTransport | None = None):
        self.base_url = base_url.rstrip("/")
        self.model = model
        self.timeout_s = timeout_s
        self.temperature = temperature
        self.seed = seed
        self.max_tokens = max_tokens
        self._transport = transport  # injetável para testes

    def _payload(self, messages: list[dict], stream: bool) -> dict:
        payload: dict = {"model": self.model, "messages": messages,
                         "temperature": self.temperature, "max_tokens": self.max_tokens,
                         "stream": stream}
        if self.seed is not None:
            payload["seed"] = self.seed
        return payload

    @staticmethod
    def _parse_completion(body: dict) -> tuple[str, dict]:
        try:
            ch = body["choices"][0]
            content = ch["message"]["content"]
            usage = body.get("usage", {})
        except (KeyError, IndexError, TypeError) as e:
            raise LlamaMalformedError(f"resposta sem choices/message: {e}") from e
        if not isinstance(content, str):
            raise LlamaMalformedError("content não é string")
        return content, usage

    async def acomplete(self, messages: list[dict]) -> tuple[str, dict]:
        try:
            async with httpx.AsyncClient(timeout=self.timeout_s,
                                         transport=self._transport) as c:
                r = await c.post(f"{self.base_url}/v1/chat/completions",
                                 json=self._payload(messages, stream=False))
        except httpx.TimeoutException as e:
            raise LlamaTimeoutError(str(e)) from e
        except httpx.HTTPError as e:
            raise LlamaConnectionError(str(e)) from e
        if r.status_code >= 400:
            raise LlamaServerError(r.status_code, r.text)
        try:
            body = r.json()
        except ValueError as e:
            raise LlamaMalformedError(str(e)) from e
        return self._parse_completion(body)

    async def astream(self, messages: list[dict]) -> AsyncIterator[tuple[str, float]]:
        """Yield (delta, t_first_delta_monotonic_no_1º). Uso: TTFC direto do server."""
        started = time.monotonic()
        ttfc_sent = False
        try:
            async with httpx.AsyncClient(timeout=self.timeout_s,
                                         transport=self._transport) as c:
                async with c.stream("POST", f"{self.base_url}/v1/chat/completions",
                                    json=self._payload(messages, stream=True)) as r:
                    if r.status_code >= 400:
                        text = (await r.aread()).decode(errors="replace")
                        raise LlamaServerError(r.status_code, text)
                    async for line in r.aiter_lines():
                        if not line.startswith("data:"):
                            continue
                        data = line[len("data:"):].strip()
                        if data == "[DONE]":
                            break
                        try:
                            chunk = json.loads(data)
                            delta = chunk["choices"][0].get("delta", {})
                        except (ValueError, KeyError, IndexError, TypeError) as e:
                            raise LlamaMalformedError(f"chunk SSE inválido: {e}") from e
                        content = delta.get("content") or ""
                        if content:
                            ts = time.monotonic()
                            if not ttfc_sent:
                                ttfc_sent = True
                                yield content, ts - started
                            else:
                                yield content, ts
        except httpx.TimeoutException as e:
            raise LlamaTimeoutError(str(e)) from e
        except httpx.HTTPError as e:
            raise LlamaConnectionError(str(e)) from e
