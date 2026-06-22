---
title: Releasing Brain Brew
---

# Releasing Brain Brew

Brain Brew releases are built with [`cargo-dist`](https://opensource.axo.dev/cargo-dist/). The release workflow produces:

- platform archives for macOS, Linux, and Windows;
- `brainbrew-installer.sh` for macOS/Linux;
- `brainbrew-installer.ps1` for Windows PowerShell;
- `brainbrew.rb` for Homebrew;
- checksums and a source archive.

## One-time GitHub setup

The generated workflow can publish the Homebrew formula to `jeprecated/homebrew-tap`. Add a repository secret named `HOMEBREW_TAP_TOKEN` to `jeprecated/brain-brew` with permission to push to `jeprecated/homebrew-tap`.

## Versioning and pinning

Brain Brew uses the Cargo workspace version as the release version. Release tags are `v<version>`, for example `v1.0.0-alpha.1`, and the Cargo workspace version must match the tag version before the tag is pushed.

Deck projects that depend on Brain Brew should pin or recommend a release tag in their contributor docs. For example, Ultimate Geography can link to the install page and say that contributors should install Brain Brew `v1.0.0-alpha.1` until the project intentionally moves to a newer Brain Brew release. CI workflows that prefer Nix can instead pin a flake revision, but that should be documented as the reproducible CI path rather than the normal user install path.

## Local release checks

Before creating a tag:

```bash
devenv shell ci
devenv shell dist:plan > /tmp/brainbrew-dist-manifest.json
devenv shell release:smoke
```

`dist:plan` verifies that `cargo-dist` can see the release package and expected artifacts for `v1.0.0-alpha.1`. `release:smoke` installs the CLI with `cargo install --path crates/brain-brew-cli --locked` into a temporary root and runs the installed binary through `validate`, `compose`, `export crowdanki`, and `verify` against the fast UG-style fixture.

If you change `dist-workspace.toml`, regenerate the workflow:

```bash
devenv shell dist:generate
```

## Cut the preview release

The current preview version is `v1.0.0-alpha.1`. The Cargo workspace version must match the tag version.

Using Jujutsu to create the tag locally:

```bash
jj tag set v1.0.0-alpha.1 -r rust-brainbrew
```

Push the tag with your Git/Jujutsu setup. The GitHub release workflow runs when the tag reaches GitHub and creates the release assets. The separate Package Smoke workflow also runs on pushes and pull requests to verify that the non-Nix Cargo install path produces a working `brainbrew` binary for `validate`, `compose`, `export`, and `verify`.

## Reviewer install commands

After the workflow completes, reviewers can install without Rust or Nix:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/jeprecated/brain-brew/releases/download/v1.0.0-alpha.1/brainbrew-installer.sh \
  | sh
brainbrew --version
```

Homebrew users can run:

```bash
brew install jeprecated/tap/brainbrew
```

Windows users can run:

```powershell
irm https://github.com/jeprecated/brain-brew/releases/download/v1.0.0-alpha.1/brainbrew-installer.ps1 | iex
brainbrew --version
```
