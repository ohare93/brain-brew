#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
Usage:
  scripts/publish_crates.sh dry-run <all|core|formats|cli>
  scripts/publish_crates.sh publish <all|core|formats|cli> --yes

Every mode first verifies Cargo-produced, extracted .crate artifacts in a staged
local Cargo source. Dependent dry-runs/publishes additionally require their
predecessor to resolve from the real crates.io index. A blocked dependent is a
failure, never a successful skipped dry-run.
USAGE
}

mode="${1:-}"
target="${2:-}"
shift $(( $# >= 2 ? 2 : $# ))
confirm="${1:-}"

case "$mode" in dry-run|publish) ;; *) usage; exit 2 ;; esac
case "$target" in all|core|formats|cli) ;; *) usage; exit 2 ;; esac
if [[ "$mode" == "publish" && "$confirm" != "--yes" ]]; then
  echo "Refusing to publish without explicit --yes." >&2
  usage
  exit 2
fi

repo_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
cd "$repo_root"
version="$(cargo metadata --no-deps --format-version 1 | python3 -c 'import json, sys; print(json.load(sys.stdin)["packages"][0]["version"])')"

declare -A package_by_target=(
  [core]="brain-brew-core"
  [formats]="brain-brew-formats"
  [cli]="brainbrew"
)
declare -A crate_by_target=(
  [core]="brain-brew-core"
  [formats]="brain-brew-formats"
  [cli]="brainbrew"
)

pre_publish_gate() {
  scripts/check_cratesio_metadata.py
  python3 scripts/verify_extracted_crates.py pre-publish
}

indexed_gate() {
  local target_name="$1"
  # This deliberately has no staged source replacement: it proves the exact
  # predecessor version is visible through real crates.io resolution.
  python3 scripts/verify_extracted_crates.py indexed --through "$target_name"
}

publish_one() {
  local target_name="$1"
  local package="${package_by_target[$target_name]}"
  if [[ "$mode" == "dry-run" ]]; then
    cargo publish -p "$package" --dry-run
  else
    cargo publish -p "$package"
  fi
}

crate_is_indexed() {
  local crate="$1"
  cargo search "$crate" --limit 5 2>/dev/null | grep -Eq "^${crate} = \"${version}\""
}

wait_for_crate() {
  local target_name="$1"
  local crate="${crate_by_target[$target_name]}"
  echo "Waiting for ${crate} ${version} to appear in the crates.io index..."
  for _ in $(seq 1 30); do
    if crate_is_indexed "$crate"; then
      echo "${crate} ${version} is visible in the crates.io index."
      return 0
    fi
    sleep 10
  done
  echo "Timed out waiting for ${crate} ${version} in the crates.io index." >&2
  echo "Do not continue: rerun the indexed gate only after the exact version appears." >&2
  return 1
}

# This packaging/build gate is required before every upload. It validates core,
# formats, and CLI in publication order against exact staged archive sources.
pre_publish_gate

case "$target" in
  core)
    publish_one core
    ;;
  formats)
    indexed_gate formats
    publish_one formats
    ;;
  cli)
    indexed_gate cli
    publish_one cli
    ;;
  all)
    publish_one core
    if [[ "$mode" == "publish" ]]; then
      wait_for_crate core
      indexed_gate formats
      publish_one formats
      wait_for_crate formats
      indexed_gate cli
      publish_one cli
    else
      echo "BLOCKED: formats and CLI dry-runs require real indexed $version predecessors." >&2
      echo "Run the individual dependent dry-runs after manually publishing and indexing core, then formats." >&2
      # A dry-run cannot create the prerequisite index entries; returning failure
      # is intentional so release automation never records this as success.
      exit 1
    fi
    ;;
esac
