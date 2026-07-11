---
title: Introduce one canonical safe-relative-path authorization module
priority: critical
---

## Goal

Reject absolute, parent, and canonical-root escapes consistently for package manifests, bases, overlays, includes, media, and writes.

## Acceptance Criteria

- All filesystem paths crossing a package/workspace root use one typed authorization interface
- Absolute and `..` paths fail before I/O
- Canonical containment is checked after existing parent/link resolution according to policy
- Diagnostics name the declaring file and offending field
- Adversarial tests cover every source route and platform-relevant path form

## Implementation Notes

First implementation task; CLI owns filesystem authorization while formats retains portable path syntax validation.


## Completion Summary

- Added portable SafeRelativePath syntax validation in formats and one CLI PathAuthorizer with typed declaration/field/value/root diagnostics
- Rejected empty, absolute, dot/parent, drive, UNC, backslash, ambiguous separator, and NUL paths before target I/O
- Added canonical existing-target and deepest-existing-parent containment for read/create authorization, including symlink/non-directory escape rejection
- Migrated package base/overlay, scalar/media includes, locked manifests, media reads/hash/copy, export/golden, Workbench source/media/new-language, and transaction target paths
- Removed implicit ancestor Workbench media discovery and documented incompatible parent-path behavior
- Added adversarial route and platform-form tests while retaining valid in-root fixtures
- Passed full fmt/test/clippy, 13 E2E, release smoke, and independent Claude judgment

### Files Changed

- crates/brain-brew-formats/src/safe_relative_path.rs
- crates/brain-brew-formats/src/lib.rs
- crates/brain-brew-formats/src/source_includes.rs
- crates/brain-brew-formats/src/source_document.rs
- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/src/overlay_source_document.rs
- crates/brain-brew-formats/tests/safe_relative_path.rs
- crates/brain-brew-cli/src/path_authorization.rs
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/io.rs
- crates/brain-brew-cli/src/media_assets.rs
- crates/brain-brew-cli/src/workspace_transaction.rs
- crates/brain-brew-cli/src/commands/lock.rs
- crates/brain-brew-cli/src/commands/media.rs
- crates/brain-brew-cli/src/commands/export.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/tests/safe_paths.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs
- documentation/docs/reference/yaml.md
- documentation/docs/reference/lockfile.md
- documentation/docs/reference/workbench.md
- documentation/docs/authoring/manifests-targets.md
- documentation/docs/authoring/media.md
- documentation/docs/authoring/workspace.md
