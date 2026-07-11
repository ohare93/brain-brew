---
title: Expose typed composition and validation errors
priority: medium
---

## Goal

Preserve validation categories and paths through composition so CLI, Workbench, and tests do not parse flattened English messages.

## Acceptance Criteria

- Composition errors retain machine-readable category, DeckPath, overlay/source attribution, and conflict metadata
- Final validation is not collapsed into one opaque variant
- CLI text and JSON renderers consume the same typed errors
- Workbench returns a versioned error envelope
- Compatibility impact on public core interfaces is documented

## Implementation Notes

After core behavior stabilizes; coordinate with typed Workbench contract.


## Completion Summary

- Added exhaustive owned `DomainDiagnostic` projections for all public composition and validation error variants, including source/overlay/path/address, expected/actual, conflicts, tombstones, FieldGraph details, and ordered child errors
- Migrated composition-backed CLI routes to the stable `diagnostics-v1` JSON envelope and typed human rendering
- Replaced Workbench English-prefix status classification and serialized-string bridges with structural typed errors and one versioned HTTP envelope
- Preserved typed diagnostics through Workbench browse, apply preview, edits, and new-language validation; development writes return 422 Domain envelopes rather than adapter 500s
- Kept lenient translation browsing usable while strict validation exposes typed stale/conflict diagnostics
- Added exhaustive projection, CLI route, Workbench API, UI unit, and browser E2E coverage for structured preview and 422 apply failures
- Refreshed checked-in Workbench assets and diagnostics/Workbench documentation
- Passed full sequential tests, default and development-write clippy, focused dev-write tests, E2E, docs, embed checks, release smoke, and Claude re-judgment

### Files Changed

- crates/brain-brew-core/src/compose.rs
- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/tests.rs
- crates/brain-brew-core/tests/overlay_compose.rs
- crates/brain-brew-cli/src/commands/compose.rs
- crates/brain-brew-cli/src/commands/explain.rs
- crates/brain-brew-cli/src/commands/export.rs
- crates/brain-brew-cli/src/commands/translation_overlay.rs
- crates/brain-brew-cli/src/commands/translations.rs
- crates/brain-brew-cli/src/commands/validate.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/output.rs
- crates/brain-brew-cli/src/planner.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-cli/tests/cli_contract.rs
- crates/brain-brew-workbench-ui/src/lib.rs
- crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs
- crates/brain-brew-cli/assets/workbench/index.html
- documentation/docs/reference/diagnostics.md
- documentation/docs/reference/cli.md
- documentation/docs/reference/workbench.md
