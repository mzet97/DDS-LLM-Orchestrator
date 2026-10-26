"""Porta uniforme de inferência (REQ-907-008/014): adapters recebem isto, não um client concreto.

Assinatura: `acomplete(messages, stage, expects=()) -> (content, usage)`.
`stage`/`expects` só são significativos para o backend determinístico (fora de banda,
campo `bench`); o LlamaPort ignora.
"""

from __future__ import annotations

from typing import Protocol, runtime_checkable

from baselines.providers.deterministic import DeterministicClient
from baselines.providers.llama import LlamaClient


@runtime_checkable
class LLMPort(Protocol):
    async def acomplete(self, messages: list[dict], stage: str,
                        expects: tuple[str, ...] = ()) -> tuple[str, dict]: ...


class LlamaPort:
    """Adapta LlamaClient (LLM real) à porta; stage/expects ignorados."""

    def __init__(self, client: LlamaClient):
        self._client = client

    async def acomplete(self, messages: list[dict], stage: str,
                        expects: tuple[str, ...] = ()) -> tuple[str, dict]:
        return await self._client.acomplete(messages)


class DeterministicPort:
    """Adapta DeterministicClient (backend fixture) à porta; mantém fora-de-banda."""

    def __init__(self, client: DeterministicClient):
        self._client = client

    async def acomplete(self, messages: list[dict], stage: str,
                        expects: tuple[str, ...] = ()) -> tuple[str, dict]:
        return await self._client.acomplete(messages, stage, list(expects))


class StageFault(RuntimeError):
    """Falha injetada 1× antes da chamada de inferência (aborta antes do backend)."""


class FaultyPort:
    """Envelope que falha `times` vezes no estágio `fail_on_stage` (recovery/retry)."""

    def __init__(self, inner: LLMPort, fail_on_stage: str, times: int = 1,
                 exc: Exception | None = None):
        self._inner = inner
        self._fail_on_stage = fail_on_stage
        self._remaining = times
        self._exc = exc
        self.faults_raised = 0

    async def acomplete(self, messages: list[dict], stage: str,
                        expects: tuple[str, ...] = ()) -> tuple[str, dict]:
        if stage == self._fail_on_stage and self._remaining > 0:
            self._remaining -= 1
            self.faults_raised += 1
            raise self._exc or StageFault(f"falta injetada em {stage}")
        return await self._inner.acomplete(messages, stage, expects)


class CallCounterPort:
    """Envelope que conta chamadas por estágio (aceite de recovery/retry)."""

    def __init__(self, inner: LLMPort):
        self._inner = inner
        self.counts: dict[str, int] = {}

    async def acomplete(self, messages: list[dict], stage: str,
                        expects: tuple[str, ...] = ()) -> tuple[str, dict]:
        self.counts[stage] = self.counts.get(stage, 0) + 1
        return await self._inner.acomplete(messages, stage, expects)


class TimingPort:
    """Envelope que registra (stage, t0, t1, ns) por chamada — métricas por estágio."""

    def __init__(self, inner: LLMPort):
        import time
        self._inner = inner
        self._time = time
        self.records: list[dict] = []

    async def acomplete(self, messages: list[dict], stage: str,
                        expects: tuple[str, ...] = ()) -> tuple[str, dict]:
        t0 = self._time.monotonic()
        content, usage = await self._inner.acomplete(messages, stage, expects)
        t1 = self._time.monotonic()
        self.records.append({"stage": stage, "t0_monotonic": t0, "t1_monotonic": t1,
                             "dur_s": t1 - t0})
        return content, usage
