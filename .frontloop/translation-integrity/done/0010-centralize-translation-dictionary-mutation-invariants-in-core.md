---
title: Centralize translation dictionary mutation invariants in core
priority: critical
---

## Goal

Replace public-map surgery duplicated in CLI and Workbench with domain commands for direct/contextual/no-change/stale/adaptation changes.

## Acceptance Criteria

- Core exposes typed commands such as set_direct, set_contextual, set_no_change, record_source_change, and resolve_stale
- Commands enforce cross-map exclusivity and precedence
- CLI and Workbench use the same policy implementation
- Property tests cover command sequences and canonical results
- Public raw-map mutation is reduced or clearly internalized

## Implementation Notes

Can start while policy decisions resolve, but final command semantics must implement them.


## Completion Summary

- Centralized dictionary mutation, validation, stale resolution, and coverage repair in core transactional APIs
- Removed legacy stale-resolution and CLI direct-map mutation bypasses
- Made field-definition identifiers structural while preserving display-variable translation
- Kept source and resolved OverlaySourceDocument views synchronized atomically
- Migrated fixture blank direct translations to explicit path-scoped adaptations
- Added atomicity, stale-resolution, view-sync, and command-sequence regression coverage
- Passed focused checks and aggregate CI; independent review required remediation then accepted the delta

### Files Changed

- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/translation.rs
- crates/brain-brew-core/src/translation_mutation.rs
- crates/brain-brew-core/tests/translation_dictionary_mutation.rs
- crates/brain-brew-core/tests/overlay_compose.rs
- crates/brain-brew-formats/src/overlay_source_document.rs
- crates/brain-brew-formats/src/source_document.rs
- crates/brain-brew-formats/src/canonical_yaml.rs
- crates/brain-brew-formats/tests/source_documents.rs
- crates/brain-brew-formats/tests/overlay_yaml.rs
- crates/brain-brew-cli/src/commands/translation_overlay.rs
- crates/brain-brew-cli/tests/cli.rs
- fixtures/ultimate-geography/overlays/languages/*.yaml
