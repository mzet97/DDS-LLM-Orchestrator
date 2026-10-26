"""Cliente do backend determinístico via Chat Completions (transporte, não orquestração)."""

from __future__ import annotations

import httpx


class DeterministicClient:
    def __init__(self, base_url: str, timeout_s: float = 30.0):
        self.base_url = base_url.rstrip("/")
        self.timeout_s = timeout_s

    def complete(self, messages: list[dict], stage: str,
                 expects: list[str] | None = None) -> tuple[str, dict]:
        with httpx.Client(timeout=self.timeout_s) as c:
            return self._do(c.post, messages, stage, expects or [])

    async def acomplete(self, messages: list[dict], stage: str,
                        expects: list[str] | None = None) -> tuple[str, dict]:
        async with httpx.AsyncClient(timeout=self.timeout_s) as c:
            return await self._ado(c.post, messages, stage, expects or [])

    def _parse(self, r: httpx.Response) -> tuple[str, dict]:
        r.raise_for_status()
        body = r.json()
        ch = body["choices"][0]
        return ch["message"]["content"], body.get("usage", {})

    def _do(self, post, messages, stage, expects):
        r = post(f"{self.base_url}/v1/chat/completions", json={
            "messages": messages, "bench": {"stage": stage, "expects": expects}})
        return self._parse(r)

    async def _ado(self, post, messages, stage, expects):
        r = await post(f"{self.base_url}/v1/chat/completions", json={
            "messages": messages, "bench": {"stage": stage, "expects": expects}})
        return self._parse(r)
