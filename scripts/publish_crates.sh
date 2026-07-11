#!/usr/bin/env bash
set -euo pipefail

usage() {
  cat >&2 <<'USAGE'
Usage:
  scripts/publish_crates.sh dry-run <all|core|formats|cli>
  scripts/publish_crates.sh publish <all|core|formats|cli> --yes

Dry-run mode is safe and is the default used by the sd release task.
Publish mode uploads immutable crates.io versions and requires --yes.
USAGE
}

mode="${1:-}"
target="${2:-}"
shift $(( $# >= 2 ? 2 : $# ))
confirm="${1:-}"

case "$mode" in
  dry-run|publish) ;;
  *) usage; exit 2 ;;
esac

case "$target" in
  all|core|formats|cli) ;;
  *) usage; exit 2 ;;
esac

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
  echo "Wait a little longer, then continue with the next package manually." >&2
  return 1
}

try_dependent_dry_run() {
  local target_name="$1"
  local package="${package_by_target[$target_name]}"
  local log
  log="$(mktemp)"
  if cargo publish -p "$package" --dry-run >"$log" 2>&1; then
    cat "$log"
    rm -f "$log"
    return 0
  fi
  if grep -Eq "no matching package named|failed to select a version for the requirement" "$log"; then
    echo "Skipping ${package} dry-run for now: exact internal dependency is not visible in crates.io yet."
    echo "After publishing earlier crates and waiting for the index, run:"
    echo "  sd release crates ${target_name}"
    rm -f "$log"
    return 0
  fi
  cat "$log" >&2
  rm -f "$log"
  return 1
}

case "$target" in
  core|formats|cli)
    publish_one "$target"
    ;;
  all)
    scripts/check_cratesio_metadata.py
    if [[ "$mode" == "dry-run" ]]; then
      publish_one core
      try_dependent_dry_run formats
      try_dependent_dry_run cli
    else
      publish_one core
      wait_for_crate core
      publish_one formats
      wait_for_crate formats
      publish_one cli
    fi
    ;;
esac
