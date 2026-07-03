#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat <<'USAGE'
Usage: scripts/sync-ug-fixture.sh [UG_CHECKOUT]

Refresh fixtures/ultimate-geography from an Ultimate Geography checkout.
UG_CHECKOUT defaults to $UG_CHECKOUT, then ../external/ultimate-geography.
USAGE
}

if [[ "${1:-}" == "-h" || "${1:-}" == "--help" ]]; then
  usage
  exit 0
fi

repo_root=$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)
ug_checkout=${1:-${UG_CHECKOUT:-../external/ultimate-geography}}
fixture_root="$repo_root/fixtures/ultimate-geography"
delta_dir="$repo_root/scripts/ug-fixture-sync"

if [[ ! -d "$ug_checkout" ]]; then
  echo "error: Ultimate Geography checkout not found: $ug_checkout" >&2
  echo "pass the checkout path explicitly, e.g. scripts/sync-ug-fixture.sh /home/jmo/Development/external/ultimate-geography" >&2
  exit 1
fi

for required in brainbrew.yaml brainbrew-hardcore.yaml deck.yaml deck-hardcore.yaml overlays descriptions templates styles; do
  if [[ ! -e "$ug_checkout/$required" ]]; then
    echo "error: missing expected UG fixture source: $ug_checkout/$required" >&2
    exit 1
  fi
done

for fragment in brainbrew.yaml.adr-0012.yaml brainbrew-hardcore.yaml.adr-0012.yaml; do
  if [[ ! -f "$delta_dir/$fragment" ]]; then
    echo "error: missing ADR-0012 delta fragment: $delta_dir/$fragment" >&2
    exit 1
  fi
done

tmp=$(mktemp -d "${fixture_root}.tmp.XXXXXX")
cleanup() {
  rm -rf "$tmp"
}
trap cleanup EXIT

# Fixture-relevant UG source files. Full media binaries are intentionally
# excluded: tests validate declared media references from deck/overlay source,
# not asset bytes. If a fixture test needs actual media bytes later, add the
# narrow path(s) here rather than copying UG's entire media directory by habit.
cp -a "$ug_checkout/brainbrew.yaml" "$tmp/"
cp -a "$ug_checkout/brainbrew-hardcore.yaml" "$tmp/"
cp -a "$ug_checkout/deck.yaml" "$tmp/"
cp -a "$ug_checkout/deck-hardcore.yaml" "$tmp/"
cp -a "$ug_checkout/overlays" "$tmp/"
cp -a "$ug_checkout/descriptions" "$tmp/"
cp -a "$ug_checkout/templates" "$tmp/"
cp -a "$ug_checkout/styles" "$tmp/"

# Reapply Brain Brew's temporary ADR-0012-only delta. Upstream UG does not yet
# carry manifest languages/translation_profile metadata, but Brain Brew's
# workbench and translation tests need the catalog while exercising real UG.
cat "$delta_dir/brainbrew.yaml.adr-0012.yaml" >> "$tmp/brainbrew.yaml"
cat "$delta_dir/brainbrew-hardcore.yaml.adr-0012.yaml" >> "$tmp/brainbrew-hardcore.yaml"

brainbrew_bin=${BRAINBREW_BIN:-}
if [[ -z "$brainbrew_bin" ]]; then
  cargo build --quiet --manifest-path "$repo_root/Cargo.toml" --bin brainbrew
  brainbrew_bin="$repo_root/target/debug/brainbrew"
elif [[ ! -x "$brainbrew_bin" ]]; then
  echo "error: BRAINBREW_BIN is not executable: $brainbrew_bin" >&2
  exit 1
fi

while IFS= read -r -d '' yaml_file; do
  "$brainbrew_bin" fmt "$yaml_file" >/dev/null
done < <(find "$tmp" -type f -name '*.yaml' -print0 | sort -z)

rm -rf "$fixture_root"
mv "$tmp" "$fixture_root"
trap - EXIT

echo "Refreshed $fixture_root from $ug_checkout"
