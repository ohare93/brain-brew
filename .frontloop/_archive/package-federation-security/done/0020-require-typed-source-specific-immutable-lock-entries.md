---
title: Require typed source-specific immutable lock entries
priority: critical
---

## Goal

Make lock verification authenticate the exact source tree by requiring canonical SRI hashes and only fields valid for each source kind.

## Acceptance Criteria

- Path, git/GitHub, and tarball locks use explicit source-specific schemas
- A valid canonical SHA-256/SRI hash is mandatory for immutable verification
- Hashless or manually weakened locks fail closed
- Lock update emits deterministic canonical entries and migration guidance
- Warm-cache verification rehashes content and rejects drift

## Implementation Notes

Depends on safe path authorization; retain current good cache rehash behavior.


## Completion Summary

- Introduced lock schema v2 with source-tagged path, Git, and tarball original/locked variants and source-inapplicable field rejection
- Required canonical SHA-256 SRI NAR hashes and immutable Git commit identities for every verified package
- Added fail-closed v1 migration diagnostics and deterministic canonical lock update output
- Rehashed live path sources on every verify and authenticated cached trees before use, rejecting warm-cache tampering
- Added malformed SRI, field-smuggling, weakened-lock, relocation, source/cache drift, and byte-idempotence regressions
- Updated lock/package/YAML documentation and fixtures
- Passed full fmt/test/clippy, focused codec/CLI tests, release smoke, and independent Claude judgment

### Files Changed

- Cargo.lock
- crates/brain-brew-formats/Cargo.toml
- crates/brain-brew-formats/src/lockfile.rs
- crates/brain-brew-formats/tests/lockfile_yaml.rs
- crates/brain-brew-formats/tests/emitter_roundtrip.rs
- crates/brain-brew-formats/tests/yaml_scalar_adversarial.rs
- crates/brain-brew-formats/tests/yaml_schema_strictness.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
- crates/brain-brew-cli/src/commands/lock.rs
- crates/brain-brew-cli/tests/lock_cli.rs
- crates/brain-brew-cli/tests/cli.rs
- documentation/docs/authoring/packages-locking.md
- documentation/docs/reference/lockfile.md
- documentation/docs/reference/yaml.md
