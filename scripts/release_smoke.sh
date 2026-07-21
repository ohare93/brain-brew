#!/usr/bin/env bash
set -euo pipefail

brainbrew_bin="${1:-brainbrew}"
repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
manifest="$repo_root/fixtures/ug-style/brainbrew.yaml"
out_dir="$(mktemp -d)"
trap 'rm -rf "$out_dir"' EXIT

"$brainbrew_bin" --version

# Every shipped binary must include the Workbench write capability while still
# requiring the explicit runtime opt-in. A missing manifest proves argument
# parsing accepted --enable-write and advanced to workspace loading.
write_probe="$out_dir/workbench-write-capability.stderr"
if "$brainbrew_bin" workbench serve \
  --manifest "$out_dir/missing-brainbrew.yaml" \
  --no-open \
  --enable-write \
  2>"$write_probe"; then
  echo "expected the Workbench write-capability probe to fail on its missing manifest" >&2
  exit 1
fi
if grep -q "built without the development-only workbench-write-dev capability" "$write_probe"; then
  echo "release binary omitted the default Workbench write capability" >&2
  exit 1
fi
grep -q "missing-brainbrew.yaml" "$write_probe"

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
