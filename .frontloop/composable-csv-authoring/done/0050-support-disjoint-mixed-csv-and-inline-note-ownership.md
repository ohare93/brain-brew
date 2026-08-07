---
title: Support disjoint mixed CSV and inline note ownership
priority: critical
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-5
---

## Goal

Enable the gradual migration promised by the design: selected notes can leave CSV ownership and become direct YAML while the rest continue to materialize from the same CSV source with identical resolved output.

## Acceptance Criteria

- Implement the ADR's composable notes source expression for direct maps, CSV sources, and mixed disjoint ownership
- Allow CSV declarations to explicitly exclude stable note IDs transferred to inline YAML
- Fail on duplicate ownership, unknown exclusions, missing transferred notes, and any implicit override attempt
- Preserve deterministic source emission without expanding the remaining imported notes
- Add a fixture that begins fully CSV-backed, moves one complete country/note to inline YAML, and proves equal CanonicalDeck semantic diff and equal adapter output
- Track per-note and per-field authoring provenance needed by later capability handling
- Use red-green-refactor, including an initial failing equivalence test and collision regressions

## Design Decisions

- Ownership transfer requires explicit exclusion from CSV plus explicit inline definition
- Source order never decides ownership
- The resolved deck before and after a pure storage migration must be semantically equal

## Implementation Notes

Depends on note materialization and preferably explicit joins. Keep source composition specific to note maps rather than introducing a general-purpose YAML merge language.


## Completion Summary

- Implemented the ADR-022 notes-only source expression for unchanged direct maps, direct `!csv`, and ordered explicitly tagged `!csv`/`!inline` sequences.
- Added strict CSV note-ID exclusions and fail-closed unknown/duplicate/missing-transfer and equal-or-unequal duplicate-ownership checks without source-order overrides.
- Preserved deterministic tagged-source emission, exclusions, inline scalar includes, and non-expanded CSV declarations.
- Added formats-owned deterministic per-note/per-field authoring provenance covering root declaration, source kind, descriptor, table/file, row, header/column, and canonical path where available.
- Added maintained CSV-to-inline storage-migration fixtures proving zero semantic diff and byte-equal CrowdAnki output plus two-CSV/inline, collision, formatting, include, and provenance regressions.
- Passed fresh Grok review, 26 focused CSV tests, 276 formats tests, full workspace tests, fmt, and clippy.

### Files Changed

- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/src/csv_note_source.rs
- crates/brain-brew-formats/src/source_document.rs
- crates/brain-brew-formats/src/source_includes.rs
- crates/brain-brew-formats/tests/csv_note_sources.rs
- crates/brain-brew-formats/tests/fixtures/csv_notes_mixed/baseline.yaml
- crates/brain-brew-formats/tests/fixtures/csv_notes_mixed/descriptor.yaml
- crates/brain-brew-formats/tests/fixtures/csv_notes_mixed/mixed.yaml
- crates/brain-brew-formats/tests/fixtures/csv_notes_mixed/notes.csv
- .frontloop/composable-csv-authoring/done/0050-support-disjoint-mixed-csv-and-inline-note-ownership.md
