#!/usr/bin/env bash
# Prepare and run the browser-only Workbench E2E package. Keep this separate
# from the deterministic workspace/package Cargo test partition.
set -euo pipefail

workspace_root="$(cd "$(dirname "${BASH_SOURCE[0]}")/.." && pwd)"
artifact_dir="${BRAINBREW_E2E_ARTIFACT_DIR:-$workspace_root/target/workbench-e2e-artifacts}"
webdriver_port="${BRAINBREW_WEBDRIVER_PORT:-9515}"
webdriver_url="${WEBDRIVER_URL:-http://127.0.0.1:${webdriver_port}}"

mkdir -p "$artifact_dir"

(
  cd "$workspace_root/crates/brain-brew-workbench-ui"
  trunk build --dist "$workspace_root/target/workbench-ui" --public-url /
)
cargo build -p brainbrew --features workbench-write-dev

export BRAINBREW_E2E_ARTIFACT_DIR="$artifact_dir"
export BRAINBREW_E2E_BIN="${BRAINBREW_E2E_BIN:-$workspace_root/target/debug/brainbrew}"
export BRAINBREW_E2E_DEV_ASSETS="${BRAINBREW_E2E_DEV_ASSETS:-$workspace_root/target/workbench-ui}"
export BRAINBREW_CHROME_BINARY="${BRAINBREW_CHROME_BINARY:-$(command -v chromium)}"
export WEBDRIVER_URL="$webdriver_url"

chromedriver --port="$webdriver_port" --log-path="$artifact_dir/chromedriver.log" &
webdriver_pid=$!
trap 'kill "$webdriver_pid" 2>/dev/null || true; wait "$webdriver_pid" 2>/dev/null || true' EXIT

for _ in $(seq 1 50); do
  if curl --silent --fail "$webdriver_url/status" > "$artifact_dir/chromedriver-status.json"; then
    break
  fi
  sleep 0.2
done
curl --silent --fail "$webdriver_url/status" > "$artifact_dir/chromedriver-status.json"

cargo test -p brain-brew-workbench-e2e -- --nocapture
