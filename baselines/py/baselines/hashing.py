"""Hash estrutural (REQ-907-014): mesma mensagens ⇒ mesmo hash, independente de sistema."""

from __future__ import annotations

import hashlib
import json


def messages_hash(messages: list[dict]) -> str:
    """SHA-256 do JSON canônico das mensagens (ordem preservada, UTF-8)."""
    canonical = json.dumps(messages, sort_keys=True, ensure_ascii=False,
                           separators=(",", ":"))
    return hashlib.sha256(canonical.encode("utf-8")).hexdigest()


def stage_request_id(workflow_id: str, step_id: str, attempt: int) -> str:
    return f"{workflow_id}-{step_id}-{attempt}"
