---
title: Implement compositional translation coverage and ownership
priority: critical
---

## Goal

Certify final target stacks across split dictionaries while attributing missing/stale content to the overlay responsible for it.

## Acceptance Criteria

- Strict coverage follows the approved ownership model across the ordered target stack
- Base and extension dictionaries can jointly satisfy completeness without hidden fallback
- Reports identify responsible source overlay, path, source text, and status
- Ignored/no-change/stale/adaptation categories remain explicit
- Main/Hardcore fixture tests cover complete and incomplete stacks

## Implementation Notes

Depends on strict-policy decision and centralized mutation commands.


## Completion Summary

- Added pure-core deterministic ordered-stack coverage with final-target completeness and introducing-overlay ownership
- Preserved stale review debt across shadowing and attributed strict diagnostics to responsible overlays
- Made deletion, adaptation, ignored, and fallback states explicit without adapter/structural inflation
- Migrated strict CLI verification to the shared coverage report
- Added Main/Hardcore, real UG manifest, deterministic ordering, and CLI ownership diagnostics tests
- Passed full CI and independent Claude judgment

### Files Changed

- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/translation.rs
- crates/brain-brew-core/tests/overlay_compose.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/tests/translations_cli.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
