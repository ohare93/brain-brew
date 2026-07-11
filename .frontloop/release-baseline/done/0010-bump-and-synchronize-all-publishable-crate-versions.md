---
title: Bump and synchronize all publishable crate versions
priority: critical
---

## Goal

Apply the approved preview version across workspace metadata, exact internal dependencies, lock data, changelog, and release configuration so no source claims the immutable alpha.1 interface.

## Acceptance Criteria

- All publishable crates and exact internal dependencies use the approved version
- Cargo.lock, changelog, dist configuration, and user-facing version references agree
- No stale alpha.1 source/interface claims remain outside historical records
- Workspace tests and metadata validation pass

## Design Decisions

- Keep publishable crates versioned in lockstep unless the release-policy task explicitly decides otherwise

## Implementation Notes

Prerequisite: clarify/choose-next-preview-version-and-supported-release-channels.


## Completion Summary

- Synchronized all publishable workspace crates and exact internal dependency constraints at 1.0.0-alpha.2
- Updated Cargo.lock, release metadata, cargo-dist configuration, GitHub release workflow, changelog, scripts, docs, README, and Nix install references
- Added deterministic release-version validation covering manifests, internal pins, lock data, derived dist/flake version sources, docs, and bounded historical alpha.1 allowlist
- Recorded crates.io as an explicit later manual publication step and labeled core/formats as implementation packages
- Removed current Homebrew claims; retained alpha.1 only as an accurate historical record
- Passed metadata validation, full tests, fmt, clippy, docs, cargo-dist plan, core package dry-run, release dry-run/smoke, Nix flake evaluation, Nix version derivation, and Claude judgment

### Files Changed

- Cargo.toml
- Cargo.lock
- crates/brain-brew-core/Cargo.toml
- crates/brain-brew-formats/Cargo.toml
- .github/workflows/release.yml
- dist-workspace.toml
- devenv.nix
- scripts/check_release_version.py
- scripts/publish_crates.sh
- CHANGELOG.md
- README.md
- documentation/docs/getting-started/install.md
- documentation/docs/reference/releasing.md
- crates/brain-brew-cli/tests/registry_planner.rs
