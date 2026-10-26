"""Testes do LlamaClient (T-907-03 / REQ-907-008): payload, erros tipados, stream."""

from __future__ import annotations

import asyncio
import json

import httpx
import pytest

from baselines.hashing import messages_hash
from baselines.providers.llama import (LlamaClient, LlamaConnectionError,
                                       LlamaMalformedError, LlamaServerError)

MESSAGES = [{"role": "system", "content": "S"}, {"role": "user", "content": "E"}]


def _ok_body(content: str = "conteudo") -> dict:
    return {"choices": [{"message": {"content": content}}],
            "usage": {"prompt_tokens": 3, "completion_tokens": 7}}


def test_acomplete_payload_e_parse():
    captured: dict = {}

    def handler(request: httpx.Request) -> httpx.Response:
        captured["json"] = json.loads(request.content)
        return httpx.Response(200, json=_ok_body("ola"))

    async def go():
        client = LlamaClient("http://mock", "gemma-test", temperature=0.0, seed=42,
                             max_tokens=256, transport=httpx.MockTransport(handler))
        return await client.acomplete(MESSAGES)

    content, usage = asyncio.run(go())
    assert content == "ola"
    assert usage["completion_tokens"] == 7
    p = captured["json"]
    assert p["model"] == "gemma-test"
    assert p["messages"] == MESSAGES
    assert p["temperature"] == 0.0
    assert p["seed"] == 42
    assert p["max_tokens"] == 256
    assert p["stream"] is False
    assert "bench" not in p  # campo exclusivo do backend determinístico


def test_erro_http_500_tipado():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(500, text="boom")

    async def go():
        client = LlamaClient("http://mock", "m", transport=httpx.MockTransport(handler))
        await client.acomplete(MESSAGES)

    with pytest.raises(LlamaServerError) as ei:
        asyncio.run(go())
    assert ei.value.status == 500


def test_erro_conexao_tipado():
    def handler(request: httpx.Request) -> httpx.Response:
        raise httpx.ConnectError("recusado")

    async def go():
        client = LlamaClient("http://mock", "m", transport=httpx.MockTransport(handler))
        await client.acomplete(MESSAGES)

    with pytest.raises(LlamaConnectionError):
        asyncio.run(go())


def test_body_malformado_tipado():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(200, json={"inesperado": True})

    async def go():
        client = LlamaClient("http://mock", "m", transport=httpx.MockTransport(handler))
        await client.acomplete(MESSAGES)

    with pytest.raises(LlamaMalformedError):
        asyncio.run(go())


def test_astream_delta_e_ttfc():
    sse = ("data: " + json.dumps({"choices": [{"delta": {"content": "a"}}]}) + "\n\n"
           + "data: " + json.dumps({"choices": [{"delta": {"content": "b"}}]}) + "\n\n"
           + "data: [DONE]\n\n")

    def handler(request: httpx.Request) -> httpx.Response:
        assert json.loads(request.content)["stream"] is True
        return httpx.Response(200, content=sse.encode())

    async def go():
        client = LlamaClient("http://mock", "m", transport=httpx.MockTransport(handler))
        deltas: list[str] = []
        tts: list[float] = []
        async for delta, t in client.astream(MESSAGES):
            deltas.append(delta)
            tts.append(t)
        return deltas, tts

    deltas, tts = asyncio.run(go())
    assert deltas == ["a", "b"]
    assert tts[0] >= 0.0 and tts[1] >= tts[0]


def test_astream_erro_tipado():
    def handler(request: httpx.Request) -> httpx.Response:
        return httpx.Response(503, text="indisponivel")

    async def go():
        client = LlamaClient("http://mock", "m", transport=httpx.MockTransport(handler))
        async for _ in client.astream(MESSAGES):
            pass

    with pytest.raises(LlamaServerError):
        asyncio.run(go())


def test_messages_hash_estavel_e_sensivel():
    h1 = messages_hash(MESSAGES)
    assert h1 == messages_hash([dict(m) for m in MESSAGES])  # cópia ⇒ mesmo hash
    assert h1 != messages_hash(MESSAGES + [{"role": "user", "content": "x"}])
