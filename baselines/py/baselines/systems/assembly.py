"""Montagem canônica de mensagens por etapa (Prompt §7): mesma regra p/ os 3 sistemas."""

from __future__ import annotations

ENTRY = "entry"
ANALYSIS = "analysis"
REVIEW = "review"
CORRECTNESS = "correctness"
SECURITY = "security"


def build_messages(system_prompt: str, entry: str,
                   prior: tuple[tuple[str, str], ...] = ()) -> list[dict]:
    """prior: tuplas (rótulo, conteúdo) — ex. (("ANALISE", a),). Ordem preservada."""
    messages = [{"role": "system", "content": system_prompt},
                {"role": "user", "content": f"ENTRADA: {entry}"}]
    for label, content in prior:
        messages.append({"role": "user", "content": f"{label}: {content}"})
    return messages


def load_prompt(prompts_dir: str, filename: str) -> str:
    with open(f"{prompts_dir}/{filename}") as f:
        return f.read()
