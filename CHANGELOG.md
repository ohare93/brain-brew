# Changelog

## v1.0.0-alpha.1

Initial Rust-based Brain Brew preview release.

- Adds the `brainbrew` CLI for Canonical Deck validation, formatting, composition, semantic diffing, and CrowdAnki import/export.
- Adds Federated Deck manifests, named targets, package-qualified composition, package locks, and CI-friendly `verify` checks.
- Adds media reference validation and release-oracle comparison support for parity reviews.
- Includes Ultimate Geography-style fixtures used to validate translations, variants, and Hardcore Geography extension overlays.
- Ships prebuilt release archives, shell and PowerShell installers, and a Homebrew formula via `cargo-dist`.
- Publishes `brain-brew-core`, `brain-brew-formats`, and `brainbrew` to crates.io so Rust users can install with `cargo install brainbrew --version 1.0.0-alpha.1 --locked`.
