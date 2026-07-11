{ inputs, pkgs, ... }:

let
  releasePkgs = import inputs."nixpkgs-cargo-dist" {
    system = pkgs.stdenv.hostPlatform.system;
  };
  # Runtime fallbacks preserve standard caller-provided overrides. Devenv runs
  # enterTest independently from shell activation, so both paths use this block.
  rustParallelismDefaults = ''
    export RUST_TEST_THREADS="''${RUST_TEST_THREADS:-2}"
    export CARGO_BUILD_JOBS="''${CARGO_BUILD_JOBS:-2}"
  '';
in
{
  packages = [
    pkgs.cargo
    pkgs.binaryen
    pkgs.chromedriver
    pkgs.chromium
    pkgs.clippy
    pkgs.curl
    pkgs.lld
    pkgs.nodejs_22
    pkgs.rustc
    pkgs.rustfmt
    pkgs.trunk
    pkgs.wasm-bindgen-cli
    releasePkgs.cargo-dist
  ];

  enterShell = ''
    ${rustParallelismDefaults}
    echo 'Brain Brew dev environment ready' > /dev/null
  '';

  scripts.brainbrew.exec = ''
    cargo run -q -p brainbrew -- "$@"
  '';
  scripts.fmt.exec = "cargo fmt --all";
  scripts."fmt:check".exec = "cargo fmt --all -- --check";
  scripts."check:rust-parallelism".exec = ''
    set -euo pipefail

    assert_parallelism() {
      local label="$1"
      local expected_test_threads="$2"
      local expected_build_jobs="$3"
      shift 3

      "$@" devenv shell -- bash -c '
        set -euo pipefail
        test "''${RUST_TEST_THREADS-}" = "$1"
        test "''${CARGO_BUILD_JOBS-}" = "$2"
      ' _ "$expected_test_threads" "$expected_build_jobs"
      printf '%s: RUST_TEST_THREADS=%s CARGO_BUILD_JOBS=%s\n' \
        "$label" "$expected_test_threads" "$expected_build_jobs"
    }

    assert_parallelism default 2 2 \
      env -u RUST_TEST_THREADS -u CARGO_BUILD_JOBS
    assert_parallelism override 7 9 \
      env -u RUST_TEST_THREADS -u CARGO_BUILD_JOBS \
      RUST_TEST_THREADS=7 CARGO_BUILD_JOBS=9
  '';
  scripts.check.exec = "cargo check --workspace --exclude brain-brew-workbench-e2e --all-targets";
  scripts.test.exec = "cargo test --workspace --exclude brain-brew-workbench-e2e --all-targets";
  scripts.clippy.exec = "cargo clippy --workspace --exclude brain-brew-workbench-e2e --all-targets -- -D warnings";
  scripts."test:workbench-write".exec = "cargo test -p brainbrew --features workbench-write-dev --test cli workbench_";
  scripts."workbench-ui-build".exec = ''
    set -euo pipefail
    cd crates/brain-brew-workbench-ui
    trunk build --dist ../../target/workbench-ui --public-url /
  '';
  scripts."workbench-ui-watch".exec = ''
    set -euo pipefail
    cd crates/brain-brew-workbench-ui
    trunk watch --dist ../../target/workbench-ui --public-url /
  '';
  scripts."workbench-ui-embed".exec = ''
    set -euo pipefail
    cd crates/brain-brew-workbench-ui
    trunk build --release --dist ../brain-brew-cli/assets/workbench --public-url /
  '';
  scripts."workbench-ui-embed-check".exec = "scripts/check_workbench_ui_embed.sh";
  # Browser-owned E2E is deliberately prepared outside the deterministic
  # workspace test partition. CI and release invoke this prepared runner.
  scripts.e2e.exec = "bash scripts/run_workbench_e2e.sh";

  scripts."docs:install".exec = "npm --prefix documentation install";
  scripts."docs:start".exec = "npm --prefix documentation run start";
  scripts."docs:build".exec = "npm --prefix documentation run build";

  scripts."dist:generate".exec = "dist generate";
  scripts."dist:plan".exec = ''
    version="$(python3 -c 'import tomllib; print(tomllib.load(open("Cargo.toml", "rb"))["workspace"]["package"]["version"])')"
    dist manifest --tag "v$version" --artifacts=all --no-local-paths --output-format=json
  '';
  scripts."release:version-check".exec = "scripts/check_release_version.py";
  scripts."crates:metadata-check".exec = ''
    set -euo pipefail
    scripts/check_release_version.py
    scripts/check_cratesio_metadata.py
  '';
  scripts."crates:verify-extracted".exec = ''
    python3 scripts/verify_extracted_crates.py pre-publish
  '';
  scripts."crates:verify-indexed".exec = ''
    python3 scripts/verify_extracted_crates.py indexed --through "''${1:-cli}"
  '';
  scripts."crates:publish-dry-run".exec = ''
    scripts/publish_crates.sh dry-run "''${1:-all}"
  '';
  scripts."crates:publish".exec = ''
    scripts/publish_crates.sh publish "''${1:-all}" --yes
  '';
  # This is the pre-upload release gate. `crates:publish-dry-run all` is
  # deliberately blocked until its real crates.io predecessors are indexed.
  scripts."release:crates".exec = "python3 scripts/verify_extracted_crates.py pre-publish";
  scripts."release:smoke".exec = ''
    set -euo pipefail
    install_root="$(mktemp -d)"
    trap 'rm -rf "$install_root"' EXIT
    cargo install --path crates/brain-brew-cli --locked --root "$install_root"
    "$PWD/scripts/release_smoke.sh" "$install_root/bin/brainbrew"
  '';

  scripts.ci.exec = ''
    set -euo pipefail
    check:rust-parallelism
    cargo fmt --all -- --check
    cargo test --workspace --exclude brain-brew-workbench-e2e --all-targets
    cargo test -p brainbrew --features workbench-write-dev --test cli workbench_
    cargo clippy --workspace --exclude brain-brew-workbench-e2e --all-targets -- -D warnings
    cargo clippy -p brainbrew --features workbench-write-dev --all-targets -- -D warnings
    workbench-ui-embed-check
  '';

  enterTest = ''
    set -euo pipefail
    ${rustParallelismDefaults}
    check:rust-parallelism
    cargo fmt --all -- --check
    cargo test --workspace --exclude brain-brew-workbench-e2e --all-targets
    cargo clippy --workspace --exclude brain-brew-workbench-e2e --all-targets -- -D warnings
  '';
}
