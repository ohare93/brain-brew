---
title: Consolidate all manifest operations behind one registry-aware planner
priority: critical
---

## Goal

Use one provenance-retaining planner for compose, verify, explain, targets text/JSON, translations, Workbench, explicit includes, package roots, and sibling locks.

## Acceptance Criteria

- Every command route applies identical package discovery, dependency, cycle, and target expansion rules
- Package-qualified targets work in text and JSON modes
- Explicit `--include` cannot bypass dependency validation
- Plans retain package/source ownership for each base, overlay, include, and media declaration
- The local-only expansion path is removed or restricted to a clearly tested private use

## Implementation Notes

After path/lock invariants; preserve deterministic order.


## Completion Summary

- Added one CLI-owned ManifestRegistry and TargetPlan planner with typed package/source/target/base/overlay/include/media provenance
- Unified root, explicit include, package-root, and sibling-lock discovery with deterministic precedence and duplicate identity rejection
- Applied identical dependency/version/cycle, qualified target lookup, target extends, and overlay expansion semantics across all manifest-backed commands
- Migrated compose, validate, verify, explain, export, targets text/JSON, translations, media, and Workbench selection/cache/media discovery
- Closed explicit --include dependency bypass and made unqualified ambiguity fail closed
- Removed CLI callers of formats-local target expansion and retained transitive source hashes/ownership for later policy enforcement
- Passed broad planner adversarial tests, full fmt/test/clippy, 13 E2E, 74+26 fixture verification, release smoke, and Claude judgment

### Files Changed

- crates/brain-brew-cli/src/planner.rs
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/io.rs
- crates/brain-brew-cli/src/package_resolver.rs
- crates/brain-brew-cli/src/path_authorization.rs
- crates/brain-brew-cli/src/commands/compose.rs
- crates/brain-brew-cli/src/commands/validate.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/src/commands/explain.rs
- crates/brain-brew-cli/src/commands/export.rs
- crates/brain-brew-cli/src/commands/targets.rs
- crates/brain-brew-cli/src/commands/translations.rs
- crates/brain-brew-cli/src/commands/media.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/tests/registry_planner.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-formats/src/source_document.rs
- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/src/overlay_source_document.rs
