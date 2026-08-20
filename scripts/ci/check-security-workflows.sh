#!/usr/bin/env bash
set -euo pipefail

root="$(git rev-parse --show-toplevel)"
workflow="$root/.github/workflows/security.yml"

fail() {
  printf 'runtime security workflow check: %s\n' "$1" >&2
  exit 1
}

test -f "$workflow" || fail "missing .github/workflows/security.yml"

while IFS= read -r use; do
  ref="${use##*@}"
  [[ "$ref" =~ ^[0-9a-f]{40}$ ]] || fail "mutable action reference: $use"
done < <(grep -hoE 'uses:[[:space:]]+[^[:space:]]+@[^[:space:]]+' "$workflow" | sed 's/^uses:[[:space:]]*//')

cache_uses="$(grep -oE 'uses:[[:space:]]+Swatinem/rust-cache@' "$workflow" | wc -l)"
uncached_targets="$(grep -oE 'cache-targets:[[:space:]]+false' "$workflow" | wc -l)"
[[ "$cache_uses" -eq "$uncached_targets" ]] || fail "every Rust cache must exclude native target artifacts"

required=(
  '^permissions:'
  'contents:[[:space:]]+read'
  'cargo fmt --all -- --check'
  'cargo clippy --workspace --all-targets --all-features --locked -- -D warnings'
  'cargo test --workspace --all-features --locked'
  'cargo audit'
  'cargo deny check'
  'cargo-audit@0\.22\.2,cargo-deny@0\.20\.2'
  'contract_real_dds'
  'topics_match_context'
  'policy_lifecycle_controls_real_dds_filesystem_side_effects'
  'concurrent_memory_claim_has_one_winner'
  'symlink'
  '04731797fd34730ab5ea9c41c8650103c841ef46'
  'miri-strict-provenance'
  'miri-disable-isolation'
  'baselines::tests::'
  'membership::tests::'
  'reproduz_numeros_do_artigo_degradado'
  'sanitizer=address'
  '--test dds_policy'
  "hashFiles\('Cargo\.lock'\)"
  'rustc -vV'
  'cache-targets:[[:space:]]+false'
)

for pattern in "${required[@]}"; do
  grep -Eq -- "$pattern" "$workflow" || fail "missing required gate: $pattern"
done

! grep -q 'continue-on-error:[[:space:]]*true' "$workflow" || fail "security checks may not continue on error"
printf 'runtime security workflow check: PASS\n'
