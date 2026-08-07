---
title: Expose CSV ownership as read-only Workbench capabilities
priority: high
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-9
---

## Goal

Let Workbench browse, compare, report, and preview CSV-backed notes and translations normally while clearly preventing unsupported writes and retaining editability for units already moved to YAML.

## Acceptance Criteria

- Workbench note, source-string, card, metadata, comparison, and coverage read paths consume materialized CSV-backed data without special output semantics
- Return source capability/provenance for relevant note fields and translation entries, including descriptor, CSV file, row, and column where available
- Disable or reject source/translation edits for CSV-owned units with a typed actionable error instead of attempting CanonicalSourceDocument YAML mutation
- Keep inline YAML-owned units in the same target writable when normal write-mode and transaction requirements are satisfied
- Include all CSV dependencies in workspace freshness and compare-and-swap input fingerprints
- Ensure Apply preview cannot stage or confirm a write that crosses into CSV ownership
- Add RGR API/UI tests proving read-only CSV rows, writable migrated YAML rows, previews, stale workspace invalidation, and fail-closed mixed apply behavior

## Design Decisions

- Read-only is per ownership unit, not necessarily per entire workspace
- CSV write-back, byte-preserving rewrites, and new-language CSV column creation remain unsupported
- Capability enforcement belongs server-side; UI disabling is not the security boundary

## Implementation Notes

Depends on provenance plus mixed note/translation ownership. Coordinate with workbench-hardening tasks, especially typed contracts, fingerprints, immutable preview, and canonical transactions. Do not broaden this task into general Workbench refactoring.


## Completion Summary

- Exposed per-unit note-source and translation capabilities with declaration, descriptor, CSV cell, and canonical-path provenance across Workbench pivots and metadata views.
- Added shared server-side preview/apply guards with typed `csv_source_read_only` and dependency-aware `csv_dependency_read_only` errors; mixed writes fail before mutation.
- Kept migrated inline units writable while separating source ownership from effective editability when a CSV translation dependency would be invalidated.
- Covered global Direct/NoChange, source AllOccurrences, metadata controls, fail-closed mixed apply, migrated inline writes, and CSV freshness/fingerprints with API/UI regressions.
- Rebuilt embedded Workbench assets and passed fresh Grok re-review, focused/full tests, fmt, clippy, UI build/embed, and browser E2E.

### Files Changed

- crates/brain-brew-cli/assets/workbench/index.html
- crates/brain-brew-cli/assets/workbench/brain_brew_workbench_ui-7d9ff9c13dedd3de.js
- crates/brain-brew-cli/assets/workbench/brain_brew_workbench_ui-7d9ff9c13dedd3de_bg.wasm
- crates/brain-brew-cli/src/commands/translations.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/src/planner.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-formats/src/csv_note_source.rs
- crates/brain-brew-formats/tests/csv_translation_sources.rs
- crates/brain-brew-workbench-ui/src/lib.rs
- documentation/docs/reference/workbench.md
- .frontloop/composable-csv-authoring/done/0090-expose-csv-ownership-as-read-only-workbench-capabilities.md
