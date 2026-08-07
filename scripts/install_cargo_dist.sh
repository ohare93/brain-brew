#!/usr/bin/env bash
# Install the reviewed cargo-dist 0.30.4 release asset only after its published
# SHA-256 matches. Pin/update procedure: documentation/docs/reference/release-security.md.
set -euo pipefail

install_dir="${1:?usage: scripts/install_cargo_dist.sh INSTALL_DIR}"
version="v0.30.4"
base_url="https://github.com/axodotdev/cargo-dist/releases/download/$version"

case "$(uname -s):$(uname -m)" in
  Linux:x86_64)
    asset="cargo-dist-x86_64-unknown-linux-gnu.tar.xz"
    sha256="f7bd986e758d0d47c6995aaf92f26d093635c7cd69581ed9e2451b618ea98098"
    binary="dist"
    ;;
  Linux:aarch64|Linux:arm64)
    asset="cargo-dist-aarch64-unknown-linux-gnu.tar.xz"
    sha256="79aa478537011e0cd4f5dd79e02f28b2b87788966d241fc605c6fe23b9e74e83"
    binary="dist"
    ;;
  Darwin:x86_64)
    asset="cargo-dist-x86_64-apple-darwin.tar.xz"
    sha256="20a1de97870d9223f003e3eb1190a04dea5d97d332238074cf980dbaf91c131d"
    binary="dist"
    ;;
  Darwin:arm64)
    asset="cargo-dist-aarch64-apple-darwin.tar.xz"
    sha256="c8b8f3163e5e4dd5a9cc5455957043a00bfee7d446489e4f8a4db6f2d5af1ab1"
    binary="dist"
    ;;
  MINGW*:*|MSYS*:*|CYGWIN*:*)
    asset="cargo-dist-x86_64-pc-windows-msvc.zip"
    sha256="c45361338770c971338a39aad168bd345ba6c2f79afd8d1c164c2db4c616ced1"
    binary="dist.exe"
    ;;
  *)
    echo "unsupported cargo-dist platform: $(uname -s):$(uname -m)" >&2
    exit 1
    ;;
esac

temporary="$(mktemp -d)"
trap 'rm -rf "$temporary"' EXIT
curl --fail --location --proto '=https' --tlsv1.2 --silent --show-error \
  --output "$temporary/$asset" "$base_url/$asset"
if command -v sha256sum > /dev/null 2>&1; then
  printf '%s  %s\n' "$sha256" "$temporary/$asset" | sha256sum --check --status
else
  # macOS runner images provide shasum rather than GNU coreutils.
  printf '%s  %s\n' "$sha256" "$temporary/$asset" | shasum -a 256 --check --status
fi

mkdir -p "$temporary/unpacked" "$install_dir"
case "$asset" in
  *.tar.xz) tar -xJf "$temporary/$asset" -C "$temporary/unpacked" ;;
  *.zip) unzip -q "$temporary/$asset" -d "$temporary/unpacked" ;;
esac
candidate="$(find "$temporary/unpacked" -type f -name "$binary" -print -quit)"
test -n "$candidate"
install -m 0755 "$candidate" "$install_dir/$binary"
