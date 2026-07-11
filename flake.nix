{
  description = "Brain Brew local-first deck federation CLI";

  inputs = {
    nixpkgs.url = "github:NixOS/nixpkgs/nixos-unstable";
  };

  outputs =
    { self, nixpkgs }:
    let
      systems = [
        "x86_64-linux"
        "aarch64-linux"
        "x86_64-darwin"
        "aarch64-darwin"
      ];
      forAllSystems = f: nixpkgs.lib.genAttrs systems (system: f system (import nixpkgs { inherit system; }));
      workspace = builtins.fromTOML (builtins.readFile ./Cargo.toml);
      version = workspace.workspace.package.version;
    in
    {
      packages = forAllSystems (
        system: pkgs:
        let
          brainbrew = pkgs.rustPlatform.buildRustPackage {
            pname = "brainbrew";
            inherit version;

            src = pkgs.lib.cleanSource ./.;
            cargoLock.lockFile = ./Cargo.lock;
            # Cargo's release test runner can expose a pseudo-terminal. Keep
            # terminal rendering unit tests byte-deterministic in the sandbox.
            BRAINBREW_COLOR = "never";

            cargoBuildFlags = [
              "-p"
              "brainbrew"
              "--bin"
              "brainbrew"
            ];
            # The distributable CLI gate owns every non-browser workspace
            # package explicitly. The WebDriver crate is browser-job-owned and
            # must never be pulled in by a package build.
            cargoTestFlags = [
              "--package"
              "brain-brew-core"
              "--package"
              "brain-brew-formats"
              "--package"
              "brainbrew"
              "--package"
              "brain-brew-workbench-ui"
              "--all-targets"
            ];

            meta = {
              description = "Local-first deck federation and round-trip CLI for Anki-compatible decks";
              homepage = "https://github.com/jeprecated/brain-brew";
              license = pkgs.lib.licenses.unlicense;
              mainProgram = "brainbrew";
            };
          };
        in
        {
          inherit brainbrew;
          default = brainbrew;
        }
      );

      apps = forAllSystems (
        system: _pkgs:
        let
          brainbrew = self.packages.${system}.brainbrew;
        in
        {
          brainbrew = {
            type = "app";
            program = "${brainbrew}/bin/brainbrew";
            meta.description = "Run the Brain Brew CLI";
          };
          default = self.apps.${system}.brainbrew;
        }
      );

      checks = forAllSystems (system: _pkgs: {
        # A real buildRustPackage build/test gate for the release CLI, not an
        # evaluation-only alias. Its explicit Cargo package partition excludes
        # only the browser-owned E2E crate.
        brainbrew = self.packages.${system}.brainbrew;
        default = self.checks.${system}.brainbrew;
      });
    };
}
