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

            cargoBuildFlags = [
              "-p"
              "brain-brew-cli"
              "--bin"
              "brainbrew"
            ];
            cargoTestFlags = [
              "--workspace"
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
        brainbrew = self.packages.${system}.brainbrew;
        default = self.checks.${system}.brainbrew;
      });

      devShells = forAllSystems (system: pkgs: {
        default = pkgs.mkShell {
          packages = [
            pkgs.cargo
            pkgs.clippy
            pkgs.rustc
            pkgs.rustfmt
          ];
        };
      });
    };
}
