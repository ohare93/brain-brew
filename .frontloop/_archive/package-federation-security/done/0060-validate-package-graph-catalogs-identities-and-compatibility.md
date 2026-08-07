---
title: Validate package graph catalogs identities and compatibility
priority: high
---

## Goal

Fail closed on package cycles, missing dependencies, incompatible base versions, and catalog entries whose declared ID/kind differs from loaded overlays.

## Acceptance Criteria

- Package dependency cycles are detected with a trace
- Missing dependencies fail through every discovery route
- Approved compatibility semantics are enforced
- Overlay catalog ID and kind match decoded overlay content
- Regression fixtures cover package-qualified overlays and duplicate package identities

## Implementation Notes

Depends on consolidated planning and compatibility decision.


## Completion Summary

- Required canonical exact SemVer dependency identities and explicit extension base_package compatibility requirements
- Implemented OR-list/AND-comparator SemVer range enforcement with documented prerelease behavior and no implicit version solving
- Validated complete package registries for missing, duplicate/conflicting, incompatible, self-cycle, and multi-package graphs with deterministic edge traces
- Decoded every catalog overlay and required catalog key/kind to match actual overlay ID/kind, rejecting aliases and qualified ownership mismatches
- Migrated fixtures and added ADR-0017, changelog, authoring/reference, lock, and downstream-package documentation
- Added discovery-route, package-qualified, cycle, identity, version/range/prerelease, and catalog mismatch regressions
- Passed full fmt/test/clippy, 13 E2E, release/docs builds, 74+26 fixture verification, and Claude judgment

### Files Changed

- CHANGELOG.md
- Cargo.toml
- Cargo.lock
- crates/brain-brew-formats/Cargo.toml
- crates/brain-brew-formats/src/package_semver.rs
- crates/brain-brew-formats/src/manifest.rs
- crates/brain-brew-formats/src/lockfile.rs
- crates/brain-brew-formats/src/lib.rs
- crates/brain-brew-formats/tests/manifest_yaml.rs
- crates/brain-brew-formats/tests/lockfile_yaml.rs
- crates/brain-brew-formats/tests/emitter_roundtrip.rs
- crates/brain-brew-formats/tests/yaml_scalar_adversarial.rs
- crates/brain-brew-formats/tests/yaml_schema_strictness.rs
- crates/brain-brew-cli/src/package_resolver.rs
- crates/brain-brew-cli/src/planner.rs
- crates/brain-brew-cli/src/output.rs
- crates/brain-brew-cli/tests/registry_planner.rs
- crates/brain-brew-cli/tests/cli.rs
- fixtures/ug-style/brainbrew.yaml
- fixtures/ultimate-geography/brainbrew.yaml
- documentation/docs/reference/decisions/0017-enforce-semantic-version-package-compatibility.md
- documentation/docs/reference/decisions/README.md
- documentation/docs/authoring/manifests-targets.md
- documentation/docs/authoring/packages-locking.md
- documentation/docs/examples/downstream-package.md
- documentation/docs/reference/lockfile.md
- documentation/docs/reference/yaml.md
