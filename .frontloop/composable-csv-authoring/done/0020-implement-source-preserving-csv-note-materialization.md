---
title: Implement source-preserving CSV note materialization
priority: critical
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-2
---

## Goal

Allow the canonical `notes` property to contain either its existing direct note map or a tagged CSV declaration that materializes ordinary validated Notes while preserving the declaration for formatting and source editing boundaries.

## Acceptance Criteria

- Add a maintained Rust CSV parser dependency in the formats crate and a typed, filesystem-free CSV note-source descriptor/materializer
- Keep the existing direct `notes:` map syntax and emitted ordering unchanged
- Parse and emit a source-preserving `notes: !csv` declaration without expanding imported rows into YAML during `fmt`
- Materialize explicit stable note IDs, one note type, scalar fields, tags, and adapter IDs into the same CanonicalDeck shape used by direct YAML
- Reject missing/invalid IDs, duplicate IDs, absent mapped headers, malformed records, incomplete fields, unknown fields, and unsupported descriptor keys with file/row/column-aware diagnostics
- Exercise quoted commas, embedded newlines, UTF-8, CRLF, empty scalar cells, deterministic row ordering, and strict schema failures
- Add failing fixture/unit tests first, make the smallest implementation pass, then refactor while keeping tests green

## Design Decisions

- The formats layer owns typed declarations and materialization; callers inject authorized source bytes
- The core model is unchanged
- Initial CSV values are read-only and deterministic
- Exact case-sensitive headers are the default

## Implementation Notes

Depends on the ADR. Primary seams: crates/brain-brew-formats/src/canonical_source_document.rs, canonical_yaml.rs, source_document.rs, source_includes.rs, and new focused tests under crates/brain-brew-formats/tests. Do not add joins, language suffixes, translation generation, or write-back in this slice.


## Completion Summary

- Added a strict filesystem-free CSV note descriptor/materializer in brain-brew-formats using the maintained csv crate.
- Preserved `notes: !csv` declarations through parse, emit, formatting, and existing structural/scalar include restoration while keeping direct note maps unchanged.
- Materialized deterministic scalar notes with stable IDs, complete fields, tags, and adapter IDs plus source/row/column-aware fail-closed diagnostics.
- Added focused RGR coverage for RFC CSV behavior, strict descriptor/declaration failures, deterministic ordering, and include/source-preservation regressions.
- Passed independent Grok review, focused/formats/source-document suites, full repository tests, formatting, and clippy.

### Files Changed

- Cargo.lock
- crates/brain-brew-formats/Cargo.toml
- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/src/csv_note_source.rs
- crates/brain-brew-formats/src/lib.rs
- crates/brain-brew-formats/src/source_includes.rs
- crates/brain-brew-formats/tests/csv_note_sources.rs
- .frontloop/composable-csv-authoring/done/0020-implement-source-preserving-csv-note-materialization.md
