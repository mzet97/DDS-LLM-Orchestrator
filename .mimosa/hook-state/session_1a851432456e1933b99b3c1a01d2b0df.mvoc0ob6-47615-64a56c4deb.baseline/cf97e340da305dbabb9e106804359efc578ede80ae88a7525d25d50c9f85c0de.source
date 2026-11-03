#!/usr/bin/env bash
# T-900-M1-02 — regenera os bindings Python do WorkflowPlanning.idl.
#
# Pré-requisitos (ver specs/900-laya-adaptive-routing/evidence/m1-01/README.md):
#  - idlc C 11.0.1 no PATH (ex.: ~/.local/cyclonedds/bin)
#  - backend `_idlpy` disponível para o `python3` do PATH
#    (sdist cyclonedds==11.0.1 compilado em venv PYTHON 3.13 +
#     CYCLONEDDS_HOME sombra com lib/ unificando lib64; o wheel do PyPI
#     NÃO traz _idlpy)
#
# Saída: idlc -l py ignora -o e escreve ./dds_llm_workflow_v1/ no CWD.
set -euo pipefail

HERE="$(cd "$(dirname "${BASH_SOURCE[0]}")" && pwd)"
IDL="$HERE/../../crates/dds-contract/idl/WorkflowPlanning.idl"
PINNED_MAIN="19b0604f82b7678706c09a7bd5046ba51783931e53834bdfa6da33755e82741f"
PINNED_INIT="9c6f11b973f2a669a91aa113608f019b1561f3a9235603781499ed67d9e30d60"

command -v idlc >/dev/null || { echo "idlc ausente no PATH" >&2; exit 1; }
tmp="$(mktemp -d)"
(cd "$tmp" && idlc -l py "$IDL")
install -d "$HERE"
rm -rf "$HERE/dds_llm_workflow_v1"
cp -r "$tmp/dds_llm_workflow_v1" "$HERE/dds_llm_workflow_v1"
rm -rf "$tmp"

main_hash="$(sha256sum "$HERE/dds_llm_workflow_v1/_WorkflowPlanning.py" | cut -d' ' -f1)"
init_hash="$(sha256sum "$HERE/dds_llm_workflow_v1/__init__.py" | cut -d' ' -f1)"
[ "$main_hash" = "$PINNED_MAIN" ] || { echo "hash _WorkflowPlanning.py divergiu: $main_hash" >&2; exit 2; }
[ "$init_hash" = "$PINNED_INIT" ] || { echo "hash __init__.py divergiu: $init_hash" >&2; exit 2; }
echo "bindings regenerados e conferidos contra os hashes pinados"
