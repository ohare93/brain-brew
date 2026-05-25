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

## Local release checks

Before creating a tag:

```bash
devbox run ci
devbox run dist:plan > /tmp/brainbrew-dist-manifest.json
```

`dist:plan` verifies that `cargo-dist` can see the release package and expected artifacts for `v1.0.0-alpha.1`.

If you change `dist-workspace.toml`, regenerate the workflow:

```bash
devbox run dist:generate
```

## Cut the preview release

The current preview version is `v1.0.0-alpha.1`. The Cargo workspace version must match the tag version.

Using Jujutsu to create the tag locally:

```bash
jj tag set v1.0.0-alpha.1 -r rust-brainbrew
```

Push the tag with your Git/Jujutsu setup. The GitHub release workflow runs when the tag reaches GitHub and creates the release assets.

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
