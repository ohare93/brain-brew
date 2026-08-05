---
title: Add explicit joins and localized-column parameters
priority: critical
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-3
---

## Goal

Support the narrow tabular composition needed by UG-style data without recreating the legacy recipe DSL: one primary table, explicitly named joined tables, and reusable localized column suffix parameters.

## Acceptance Criteria

- Support table aliases with one explicit primary table and ordered joins that each declare both join columns
- Support a typed localized-column rule whose parameter defaults to empty and otherwise appends the configured separator plus language value
- Allow field and adapter-ID mappings to opt into localized suffixing while shared fields such as tags and media remain unsuffixed
- Reject duplicate primary or joined keys, missing required joined rows, ambiguous column ownership, unexpected column collisions, invalid parameters, and pathologically recursive/implicit join declarations
- Keep materialization deterministic regardless of filesystem or hash-map order
- Add a compact multi-file fixture shaped like main/country/guid data and cover empty, `de`, and `zh-tw` parameter values
- Use red-green-refactor for each join and parameter behavior, retaining regression tests for every strict failure

## Design Decisions

- Only explicit keyed flat joins are supported; legacy recursive derivative inference is out of scope
- Localized suffix construction is a typed source parameter, not general expression evaluation and not a deck text variable
- Missing and duplicate join semantics fail closed

## Implementation Notes

Depends on CSV note materialization. Keep the descriptor narrowly sufficient for tabular note data; no formula language, automatic header lowercasing, arbitrary transforms, or write-back.


## Completion Summary

- Extended the existing CSV materializer with explicit flat many-to-one joins over one primary table and uniquely keyed joined tables.
- Added required and optional join semantics, strict key ownership/cardinality checks, and fail-closed rejection of chained, recursive, implicit, or ambiguous declarations.
- Added literal `localized_column` parameters with empty defaults and explicit field/adapter-ID opt-in while keeping tags unsuffixed.
- Added maintained main/country/guid fixtures for empty, `de`, and `zh-tw` parameter values plus strict regression coverage for join and parameter failures.
- Passed independent Grok review, 14 focused CSV tests, the full formats and workspace suites, formatting, and clippy.

### Files Changed

- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/src/csv_note_source.rs
- crates/brain-brew-formats/tests/csv_note_sources.rs
- crates/brain-brew-formats/tests/fixtures/csv_notes_joins/descriptor.yaml
- crates/brain-brew-formats/tests/fixtures/csv_notes_joins/main.csv
- crates/brain-brew-formats/tests/fixtures/csv_notes_joins/country.csv
- crates/brain-brew-formats/tests/fixtures/csv_notes_joins/guid.csv
- .frontloop/composable-csv-authoring/done/0030-add-explicit-joins-and-localized-column-parameters.md
