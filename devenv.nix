{ inputs, pkgs, ... }:

let
  releasePkgs = import inputs."nixpkgs-cargo-dist" {
    system = pkgs.stdenv.hostPlatform.system;
  };
in
{
  packages = [
    pkgs.cargo
    pkgs.clippy
    pkgs.nodejs_22
    pkgs.rustc
    pkgs.rustfmt
    releasePkgs.cargo-dist
  ];

  enterShell = ''
    echo 'Brain Brew dev environment ready' > /dev/null
  '';

  scripts.brainbrew.exec = ''
    cargo run -q -p brainbrew -- "$@"
  '';
  scripts.fmt.exec = "cargo fmt --all";
  scripts."fmt:check".exec = "cargo fmt --all -- --check";
  scripts.check.exec = "cargo check --workspace --all-targets";
  scripts.test.exec = "cargo test --workspace --all-targets";
  scripts.clippy.exec = "cargo clippy --workspace --all-targets -- -D warnings";

  scripts."docs:install".exec = "npm --prefix documentation install";
  scripts."docs:start".exec = "npm --prefix documentation run start";
  scripts."docs:build".exec = "npm --prefix documentation run build";

  scripts."dist:generate".exec = "dist generate";
  scripts."dist:plan".exec = ''
    dist manifest --tag v1.0.0-alpha.1 --artifacts=all --no-local-paths --output-format=json
  '';

  scripts.ci.exec = ''
    set -euo pipefail
    cargo fmt --all -- --check
    cargo test --workspace --all-targets
    cargo clippy --workspace --all-targets -- -D warnings
  '';

  enterTest = ''
    set -euo pipefail
    cargo fmt --all -- --check
    cargo test --workspace --all-targets
    cargo clippy --workspace --all-targets -- -D warnings
  '';
}
