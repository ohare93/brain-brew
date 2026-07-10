#!/usr/bin/env bash
set -euo pipefail

brainbrew_bin="${1:-brainbrew}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$repo_root/fixtures/ug-style/brainbrew.yaml"
out_dir="$(mktemp -d)"
trap 'rm -rf "$out_dir"' EXIT

"$brainbrew_bin" --version

"$brainbrew_bin" validate \
  --manifest "$manifest" \
  --target full-demo

"$brainbrew_bin" compose \
  --manifest "$manifest" \
  --target full-demo \
  --out "$out_dir/full-demo.yaml"
test -s "$out_dir/full-demo.yaml"

# The fast ug-style fixture intentionally omits media bytes and hashes. This is
# structural/reference coverage only, never release media-integrity evidence.
"$brainbrew_bin" export crowdanki \
  --manifest "$manifest" \
  --target full-demo \
  --media-mode reference-only \
  --out "$out_dir/crowdanki"
test -s "$out_dir/crowdanki/deck.json"

"$brainbrew_bin" verify \
  --manifest "$manifest" \
  --all-targets \
  --media-mode reference-only
