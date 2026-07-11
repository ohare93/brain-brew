---
title: Resolve structured messages as a validated dependency graph
priority: high
---

## Goal

Replace stale-snapshot rendering with deterministic dependency-order resolution and explicit missing-reference/cycle failures.

## Acceptance Criteria

- Multi-hop references resolve from current upstream values
- Cycles are detected with a useful path trace
- Missing variables and swallowed render errors are surfaced consistently
- Resolution is deterministic regardless of map insertion order
- Tests cover chains, diamonds, cycles, mixed images/messages, and overlay updates

## Implementation Notes

Build on semantic field values; keep core pure.


## Completion Summary

- Added one pure per-note FieldGraph planner/resolver shared by validation, rendering, translation, CrowdAnki, CLI verify, and Workbench
- Resolved scalar/message/image chains from final semantic values with dependency-first memoization and once-only diamond lowering
- Added typed missing note/definition/value, tombstoned dependency, invalid target/image/message/reference, and cycle diagnostics
- Canonicalized closed cycle traces independent of map insertion, root traversal, and unrelated fields
- Removed stale snapshot rendering, one-hop resolution, duplicate validation paths, and swallowed Workbench render fallbacks
- Preserved reusable structured translation reference edges so later upstream overlays update downstream output
- Added comprehensive chain/diamond/self/multi/tail-cycle/missing/image/overlay/translation/insertion-order tests and documentation
- Passed CPU-bounded full tests including UG 74+26, clippy, serialized 13-test E2E, docs, release smoke, and Claude judgment

### Files Changed

- crates/brain-brew-core/src/messages.rs
- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/validate.rs
- crates/brain-brew-core/src/compose.rs
- crates/brain-brew-core/src/translation.rs
- crates/brain-brew-core/tests/message_graph.rs
- crates/brain-brew-core/tests/overlay_compose.rs
- crates/brain-brew-core/tests/translation_coverage.rs
- crates/brain-brew-formats/tests/canonical_yaml.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
- crates/brain-brew-cli/src/commands/translation_overlay.rs
- crates/brain-brew-cli/src/commands/translations.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- documentation/docs/authoring/translations.md
- documentation/docs/reference/yaml.md
