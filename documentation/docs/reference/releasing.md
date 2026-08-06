---
title: Releasing Brain Brew
---

# Releasing Brain Brew

Brain Brew releases are built with [`cargo-dist`](https://opensource.axo.dev/cargo-dist/). The release workflow produces platform archives for macOS, Linux, and Windows; shell and PowerShell installers; checksums; and a source archive.

## Versioning, channels, and pinning

`workspace.package.version` in `Cargo.toml` is the authoritative release-version source. Publishable crates inherit it, and their packaged internal dependencies use the exact workspace requirement. The current preview is `1.0.0-alpha.5`; its tag is `v1.0.0-alpha.5`.

The supported channels are manual crates.io publication after the release gates, pinned GitHub release artifacts, and a pinned Nix flake/tag channel. Deck projects should recommend the release tag; Nix consumers should use `github:jeprecated/brain-brew/v1.0.0-alpha.5` or lock its resolved revision. The `brain-brew-core` and `brain-brew-formats` crates are published implementation packages, not supported public Rust APIs; the CLI install surface has the preview compatibility commitment.

## Preview compatibility promise

For this crates.io preview, the compatibility promise covers Canonical Deck YAML, overlay YAML, manifest targets for a single package, deck and overlay composition semantics, and the core CLI verbs: `fmt`, `validate`, `compose`, `export crowdanki`, `import crowdanki`, `diff`, `explain`, `targets`, `translations`, `media`, and `verify`.

The lock/package federation surface is explicitly outside that promise.

> **Experimental:** Lock/package federation works today, but the `brainbrew.lock` format and `brainbrew lock` CLI surface may change incompatibly in any release until a real downstream consumer stabilizes them.

## Local release checks

Before creating the tag, run:

```bash
devenv shell ci
devenv shell e2e
nix build .#checks.x86_64-linux.brainbrew -L
devenv shell crates:metadata-check
devenv shell dist:plan > /tmp/brainbrew-dist-manifest.json
devenv shell release:smoke
devenv shell release:crates
```

`nix build .#checks.x86_64-linux.brainbrew` is the real release-CLI build gate: it builds the distributable binary and explicitly tests every non-browser workspace package. It must stay independent from `workbench-e2e`; do not add WebDriver filters or browser dependencies to it. `devenv shell e2e` is the separate prepared Linux browser gate. It builds the write-enabled test CLI, fresh UI assets, Chromium, and chromedriver before running all browser scenarios. It is required by CI and the release workflow, but is not a supported Nix/Darwin check. On a local failure, inspect `target/workbench-e2e-artifacts/`; CI uploads that directory regardless of outcome.

`crates:metadata-check` verifies version references across Cargo metadata/lock data, dist planning, flake derivation, and current release docs, then verifies crates.io metadata and exact internal requirements. `release:crates` runs the required pre-publish artifact gate: Cargo creates fresh `.crate` files, the gate enumerates each archive's README, license, generated files, and symlinks, safely extracts its exact bytes into a clean temporary tree, and runs offline tests for core → formats → CLI. Repository-only integration tests are explicitly excluded from crates.io packages rather than silently packaging unavailable root fixtures. Formats and CLI resolve exact alpha.5 internal dependencies from a checksum-verified staged Cargo directory source made only from those extracted archives; no workspace path is evidence. Evidence JSON is retained under `target/release-evidence/`.

This is the **pre-publish coherence** mode and is the only complete dependency-chain check available before manual upload. `dist:plan` derives its tag from `Cargo.toml`. `release:smoke` now creates Cargo-produced archives, safely extracts them, compiles the CLI against the staged extracted archives, copies that produced binary to an isolated install root, and runs `validate`, `compose`, `export crowdanki`, and `verify` through that binary. It never uses `cargo install --path` or a workspace binary as package evidence. The fast UG-style fixture used by this smoke has `--media-mode reference-only`; retain it as fixture evidence only, never as live-consumer or media-integrity evidence.

The separate **indexed** mode uses no staged source replacement: it builds the extracted dependents while resolving predecessors from real crates.io. Run it after each earlier package appears in the index. It is intentionally blocked (and exits nonzero) when alpha.5 is absent; the immutable `brain-brew-core` alpha.1 is incompatible and must never be accepted as a substitute. `crates:publish-dry-run all` likewise reports this blocked state instead of silently skipping dependent verification.

### Reusable GitHub release gate

CI and tagged releases both call `.github/workflows/reusable-quality.yml` with a full 40-character commit SHA. The workflow rejects refs, checks out only that SHA, and returns that verified SHA plus a SHA-256-bound evidence bundle. It runs format/tests/lint/embedded assets, documentation, extracted crates, archive-only package smoke, a produced cargo-dist archive smoke, the Nix check and produced Nix binary smoke, and prepared browser E2E. Tagged releases verify that the tag resolves to this same SHA; every cargo-dist build artifact upload and the only `dist host`/`gh release create` path depends on the reusable gate. A failed, cancelled, or skipped dependency leaves hosting skipped by normal GitHub Actions semantics—there is no `always()` publication bypass.

Pull requests run this exact gate at their immutable head SHA with read-only permissions and no publication credentials. They record a `blocked` representative-consumer result rather than treating the checked-in Ultimate Geography fixture as equivalent evidence. See [Release supply-chain security](./release-security.md) for the exact action pins, checksum-verified cargo-dist bootstrap, locked Nix installation, host-only credential boundary, and mandatory pin-update review process. Run its checker before release changes:

```bash
python3 scripts/check_release_security.py
python3 -m unittest scripts.tests.test_release_security_policy
```

### Live representative-consumer blocker and recovery

**Current blocker:** no live Ultimate Geography consumer integration is configured, so every tag release intentionally fails closed before cargo-dist hosting. To unblock a future release, the separately owned live-consumer workflow must publish HTTPS JSON evidence with `schema_version: 1`, `status: "passed"`, `consumer: "ultimate-geography-live"`, the exact `target_sha`, a non-empty `artifact_sha256`, and its executed `commands`. Set the repository variables `REPRESENTATIVE_CONSUMER_EVIDENCE_URL` and `REPRESENTATIVE_CONSUMER_EVIDENCE_SHA256` to its HTTPS URL and exact lowercase SHA-256. The reusable gate downloads and verifies those bytes and refuses wrong SHA, wrong consumer, fixture substitution, malformed evidence, or an absent checksum. Do not set those variables to fixture output.

### Artifact-gate recovery and cleanup

The verifier never uploads and removes its temporary archives, extracted trees, and staged source registry on exit. Keep the JSON evidence under `target/release-evidence/` with the release review; remove that ignored directory after the review if it is no longer needed. The reusable workflow uploads its SHA-bound evidence bundle under `quality-evidence-<target-sha>` on successful gates. If pre-publish coherence fails, fix the packaged manifest/interface/archive material and rerun `devenv shell release:crates` before any upload. If indexed verification is blocked, do **not** republish an immutable predecessor: wait for its exact alpha.5 index entry, run `devenv shell crates:verify-indexed formats` (or `cli`), then continue in order.

GitHub Release/Nix artifacts are independent of crates.io indexing. A green cargo-dist build or Nix build does not satisfy this crate registry gate, and this manual crates.io sequence does not create a GitHub Release.

If you change `dist-workspace.toml`, regenerate the workflow:

```bash
devenv shell dist:generate
```

## Cut the preview release

The workspace version must match the tag before it is pushed:

```bash
jj tag set v1.0.0-alpha.5 -r rust-brainbrew
```

Push the tag with your Git/Jujutsu setup only after the gates pass. The GitHub workflow creates pinned release artifacts; manual crates.io publication remains a separate later step.

## Publish crates.io packages

Log in once with `cargo login`, then publish in dependency order. Crates.io versions are immutable, so double-check the workspace version, README snippets, and changelog first.

```bash
devenv shell crates:publish-dry-run core
devenv shell crates:publish core
# wait for brain-brew-core v1.0.0-alpha.5 in the crates.io index

devenv shell crates:publish-dry-run formats
devenv shell crates:publish formats
# wait for brain-brew-formats v1.0.0-alpha.5 in the crates.io index

devenv shell crates:publish-dry-run cli
devenv shell crates:publish cli
```

The same commands are backed by `scripts/publish_crates.sh`. Every `publish` invocation reruns pre-publish artifact verification before upload; formats and CLI also require the indexed check. `release:crates` is the pre-upload extracted-artifact gate, while `crates:publish-dry-run all` is deliberately blocked until the real predecessor versions exist. No command in this repository should publish without the explicit manual release decision.

## Reviewer install commands

After alpha.5 is released, reviewers can install the locked Cargo package or
Nix target. For a GitHub installer, use the checksum-verified download procedure
in [Install the CLI](../getting-started/install.md#github-release-installer);
do not pipe a remote installer into a shell.

```bash
cargo install brainbrew --version 1.0.0-alpha.5 --locked
nix run github:jeprecated/brain-brew/v1.0.0-alpha.5 -- --version
```
