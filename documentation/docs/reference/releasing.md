---
title: Releasing Brain Brew
---

# Releasing Brain Brew

Brain Brew releases are built with [`cargo-dist`](https://opensource.axo.dev/cargo-dist/). The release workflow produces platform archives for macOS, Linux, and Windows; shell and PowerShell installers; checksums; and a source archive.

## Versioning, channels, and pinning

`workspace.package.version` in `Cargo.toml` is the authoritative release-version source. Publishable crates inherit it, and their packaged internal dependencies use the exact workspace requirement. The current preview is `1.0.0-alpha.2`; its tag is `v1.0.0-alpha.2`.

The supported channels are manual crates.io publication after the release gates, pinned GitHub release artifacts, and a pinned Nix flake/tag channel. Deck projects should recommend the release tag; Nix consumers should use `github:jeprecated/brain-brew/v1.0.0-alpha.2` or lock its resolved revision. The `brain-brew-core` and `brain-brew-formats` crates are published implementation packages, not supported public Rust APIs; the CLI install surface has the preview compatibility commitment.

## Preview compatibility promise

For this crates.io preview, the compatibility promise covers Canonical Deck YAML, overlay YAML, manifest targets for a single package, deck and overlay composition semantics, and the core CLI verbs: `fmt`, `validate`, `compose`, `export crowdanki`, `import crowdanki`, `diff`, `explain`, `targets`, `translations`, `media`, and `verify`.

The lock/package federation surface is explicitly outside that promise.

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

## Local release checks

Before creating the tag, run:

```bash
devenv shell ci
devenv shell crates:metadata-check
devenv shell dist:plan > /tmp/brainbrew-dist-manifest.json
devenv shell release:smoke
devenv shell release:crates
```

`crates:metadata-check` verifies version references across Cargo metadata/lock data, dist planning, flake derivation, and current release docs, then verifies crates.io metadata and exact internal requirements. `dist:plan` derives its tag from `Cargo.toml`. `release:smoke` installs the CLI from the workspace into a temporary root and checks `validate`, `compose`, `export crowdanki`, and `verify` against the fast UG-style fixture. That fixture uses `--media-mode reference-only`; it is not release media-integrity evidence.

Only `brain-brew-core` can fully dry-run before publication because dependents resolve exact internal dependencies from crates.io. After each earlier crate is visible in the index, dry-run and then publish the next crate in dependency order.

If you change `dist-workspace.toml`, regenerate the workflow:

```bash
devenv shell dist:generate
```

## Cut the preview release

The workspace version must match the tag before it is pushed:

```bash
jj tag set v1.0.0-alpha.2 -r rust-brainbrew
```

Push the tag with your Git/Jujutsu setup only after the gates pass. The GitHub workflow creates pinned release artifacts; manual crates.io publication remains a separate later step.

## Publish crates.io packages

Log in once with `cargo login`, then publish in dependency order. Crates.io versions are immutable, so double-check the workspace version, README snippets, and changelog first.

```bash
devenv shell crates:publish-dry-run core
devenv shell crates:publish core
# wait for brain-brew-core v1.0.0-alpha.2 in the crates.io index

devenv shell crates:publish-dry-run formats
devenv shell crates:publish formats
# wait for brain-brew-formats v1.0.0-alpha.2 in the crates.io index

devenv shell crates:publish-dry-run cli
devenv shell crates:publish cli
```

The same commands are backed by `scripts/publish_crates.sh`; `release:crates` is dry-run only. No command in this repository should publish without the explicit manual release decision.

## Reviewer install commands

After alpha.2 is released, reviewers can install pinned artifacts:

```bash
curl --proto '=https' --tlsv1.2 -LsSf \
  https://github.com/jeprecated/brain-brew/releases/download/v1.0.0-alpha.2/brainbrew-installer.sh \
  | sh
cargo install brainbrew --version 1.0.0-alpha.2 --locked
nix run github:jeprecated/brain-brew/v1.0.0-alpha.2 -- --version
```
