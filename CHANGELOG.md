# Changelog

## Unreleased

- Package federation now validates the complete root/include/package-root/lock registry before planning: package versions and exact pins use SemVer, explicit base compatibility ranges are enforced, dependency cycles include full deterministic edge traces, and overlay catalog IDs/kinds must match decoded sources.
- CrowdAnki import now fails closed on additional unsupported or ambiguous data, including `bafmt`/`bqfmt`/`did` fields and media stable-ID collisions, while preserving media-file parity.
- Canonical YAML emission is safer for block scalars, CRLF-sensitive content, and map keys, with validated/quoted keys and hard failures for un-emittable key text.
- Overlay field operations are fill-blank-only, and full `note:`/`note_type:` bodies now fail closed unless they are explicit `add` changes.
- Package verification detects drift in local path sources, and path locks are stored as portable paths relative to `brainbrew.lock`.
- Translation coverage now surfaces untranslated structured-message `format` glue strings, and stale-translation resolution removes shadowed stale records cleanly.
- `validate --json`, `explain --json`, `diff --json`, and `targets --json` now emit machine-parseable failure envelopes on stdout with non-zero exits, empty stderr, and structured `error.errors[]` details where available.

## v1.0.0-alpha.1

Initial Rust-based Brain Brew preview release.

- Adds the `brainbrew` CLI for Canonical Deck validation, formatting, composition, semantic diffing, and CrowdAnki import/export.
- Adds Federated Deck manifests, named targets, package-qualified composition, package locks, and CI-friendly `verify` checks.
- Adds media reference validation and release-oracle comparison support for parity reviews.
- Includes Ultimate Geography-style fixtures used to validate translations, variants, and Hardcore Geography extension overlays.
- Ships prebuilt release archives, shell and PowerShell installers, and a Homebrew formula via `cargo-dist`.
- Publishes `brain-brew-core`, `brain-brew-formats`, and `brainbrew` to crates.io so Rust users can install with `cargo install brainbrew --version 1.0.0-alpha.1 --locked`.
