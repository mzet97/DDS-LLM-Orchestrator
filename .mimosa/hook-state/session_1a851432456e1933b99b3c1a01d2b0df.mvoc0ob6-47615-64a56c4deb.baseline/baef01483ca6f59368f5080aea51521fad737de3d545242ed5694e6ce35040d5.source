"""Monta um diretório de checkpoint Laya COMPLETO a partir de pesos + complemento (T904-M3).

O SDK `laya==0.4.10` só carrega diretórios com `model.safetensors` +
`rl_agent_config.json` + `encoder/config.json` + `tokenizer/*.json`
(agent.py). Os pesos treinados preservados localmente (T900/T902/T903) estão
SOZINHOS — este tool monta um diretório operacional (default: symlinks,
zero-cópia) com o complemento arquitetural (não treinado) de um checkpoint
completo de referência (ex.: snapshot do BASE no cache HF) e registra TODOS
os hashes em `MANIFEST.assembly.json`.

Segurança:
- `--expected-sha256` obrigatório: os pesos só entram se o hash conferir;
- NUNCA escreve dentro dos diretórios de origem (artefatos canônicos são
  somente leitura);
- idempotente: reexecução re-verifica e recria links ausentes/quebrados;
  recusa sobrescrever destino que não seja o próprio link.
"""

from __future__ import annotations

import argparse
import hashlib
import json
import sys
import time
from pathlib import Path

COMPANIONS: dict[str, str] = {
    "rl_agent_config.json": "arquitetura (complemento, não treinada)",
    "encoder/config.json": "encoder (complemento, não treinada)",
    "tokenizer/tokenizer_config.json": "tokenizador (complemento, não treinada)",
    "tokenizer/tokenizer.json": "tokenizador (complemento, não treinada)",
}
REQUIRED = ("model.safetensors",) + tuple(COMPANIONS)
MANIFEST_NAME = "MANIFEST.assembly.json"


def sha256_file(path: Path) -> str:
    h = hashlib.sha256()
    with open(path, "rb") as f:
        for chunk in iter(lambda: f.read(1 << 20), b""):
            h.update(chunk)
    return h.hexdigest()


def _entry(src: Path, role: str, mode: str, dst: Path) -> dict:
    return {
        "source": str(src),
        "sha256": sha256_file(src),
        "bytes": src.stat().st_size,
        "role": role,
        "mode": mode,
        "link": str(dst) if mode == "link" else "",
    }


def main() -> int:
    ap = argparse.ArgumentParser(prog="assemble_checkpoint")
    ap.add_argument("--weights", required=True,
                    help="model.safetensors treinado (ex.: T900)")
    ap.add_argument("--complement", required=True,
                    help="diretório de checkpoint completo de referência (ex.: snapshot BASE no cache HF)")
    ap.add_argument("--out", required=True, help="diretório operacional a montar")
    ap.add_argument("--expected-sha256", required=True,
                    help="sha256 esperado dos pesos (recusa divergência)")
    ap.add_argument("--mode", choices=("link", "copy"), default="link",
                    help="link = symlinks zero-cópia (default); copy = cópia física")
    args = ap.parse_args()

    weights = Path(args.weights).resolve()
    complement = Path(args.complement).resolve()
    out = Path(args.out).resolve()

    if not weights.is_file():
        print(f"ERRO: pesos ausentes: {weights}", file=sys.stderr)
        return 2
    if not complement.is_dir():
        print(f"ERRO: complemento ausente: {complement}", file=sys.stderr)
        return 2
    for outp, name in ((weights.parent, "pesos"), (complement, "complemento")):
        if out == outp or out in outp.parents or outp in out.parents:
            print(f"ERRO: --out não pode conter/estar dentro da origem ({name}): {outp}",
                  file=sys.stderr)
            return 2

    got = sha256_file(weights)
    if got != args.expected_sha256.lower():
        print(f"ERRO: hash dos pesos diverge do esperado: {got} != {args.expected_sha256}",
              file=sys.stderr)
        return 2

    missing = [rel for rel in COMPANIONS if not (complement / rel).is_file()]
    if missing:
        print(f"ERRO: complemento incompleto em {complement}; faltam: {', '.join(missing)}",
              file=sys.stderr)
        return 2

    out.mkdir(parents=True, exist_ok=True)
    entries: dict[str, dict] = {}

    for rel, role in (("model.safetensors", "pesos treinados (hash verificado)"),) + tuple(COMPANIONS.items()):
        src = weights if rel == "model.safetensors" else complement / rel
        dst = out / rel
        dst.parent.mkdir(parents=True, exist_ok=True)
        if dst.is_symlink():
            if dst.resolve() == src:
                pass  # já correto
            else:
                dst.unlink()
        elif dst.exists():
            print(f"ERRO: destino existe e não é nosso link — recusado: {dst}", file=sys.stderr)
            return 2
        if not dst.exists():
            if args.mode == "link":
                dst.symlink_to(src)
            else:
                import shutil

                shutil.copy2(src, dst, follow_symlinks=True)
        entries[rel] = _entry(src, role, args.mode, dst)

    if entries["model.safetensors"]["sha256"] != args.expected_sha256.lower():
        print("ERRO: verificação pós-montagem falhou", file=sys.stderr)
        return 2

    manifest = {
        "tool": "workers/laya-worker/tools/assemble_checkpoint.py",
        "created_at": time.strftime("%Y-%m-%dT%H:%M:%S%z"),
        "mode": args.mode,
        "expected_sha256": args.expected_sha256.lower(),
        "weights": {"path": str(weights), "sha256": got, "bytes": weights.stat().st_size},
        "complement": {"path": str(complement)},
        "entries": entries,
        "note": "complemento = arquitetura/tokenização do checkpoint de referência "
                "(não treinada); pesos são o artefato canônico apontado",
    }
    (out / MANIFEST_NAME).write_text(json.dumps(manifest, indent=1, ensure_ascii=False))

    print(json.dumps({
        "out": str(out),
        "weights_sha256": got,
        "entries": sorted(entries),
        "manifest": str(out / MANIFEST_NAME),
    }))
    return 0


if __name__ == "__main__":
    sys.exit(main())
