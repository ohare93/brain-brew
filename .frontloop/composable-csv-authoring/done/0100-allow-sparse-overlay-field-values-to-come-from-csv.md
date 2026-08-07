---
title: Allow sparse overlay field values to come from CSV
priority: high
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-10
---

## Goal

Cover data such as UG's Experimental region-code column without duplicating it into generated YAML by allowing a narrowly scoped CSV source to materialize sparse field-addition values in an extension overlay.

## Acceptance Criteria

- Add a source-preserving CSV declaration at the existing field-addition values boundary rather than a generic legacy recipe mechanism
- Map explicit stable note IDs to one or more explicitly declared added fields using the same descriptor, joins, parameters, authorization, and provenance rules
- Validate note existence, field-definition ownership, completeness policy, value types, duplicate ownership, and expected-base/composition invariants through existing overlay semantics
- Preserve direct inline field-addition values and permit disjoint gradual ownership transfer with fatal collisions
- Add a fixture for an Experimental-style region-code field and prove equal composition before and after moving selected values to YAML
- Ensure formatting preserves the declaration and Workbench capabilities report CSV ownership correctly
- Use red-green-refactor and retain regression tests for sparse rows, unknown notes, collisions, and output equality

## Design Decisions

- This is a narrow reuse of CSV source materialization at a real Canonical overlay seam
- Do not implement recursive derivatives, arbitrary transforms, note-model selection, CSV generation, or the old recipe DSL

## Implementation Notes

Depends on the shared descriptor/materializer, CLI provenance, and mixed ownership rules. Keep this after base notes and translations so the abstraction is proven by concrete consumers rather than generalized speculatively.


## Completion Summary

- Added source-preserved `field_additions.<note-type>.values.from_csv` declarations reusing strict descriptors, joins, parameters, typed mappings, authorization, and provenance.
- Materialized only non-empty sparse cells for fields added by the owning extension, with strict note/field/type/join/duplicate/collision checks and exact typed inline transfer equivalence.
- Integrated sparse-first, translation-second complete source-shape materialization in manifest and ad-hoc compose/export/validate paths, including all descriptor/table fingerprints and freshness inputs.
- Exposed sparse CSV ownership to Workbench shared read-only guards while routing excluded migrated inline values back to their owning extension overlay.
- Added Experimental region-code before/after fixtures proving equal canonical composition and CrowdAnki output plus sparse, join, transfer, ordering, formatting, authorization, and Workbench regressions.
- Passed fresh Grok re-review, focused suites, full workspace tests, fmt, and clippy.

### Files Changed

- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/src/io.rs
- crates/brain-brew-cli/src/planner.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-cli/tests/csv_authoring_sources.rs
- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/src/csv_note_source.rs
- crates/brain-brew-formats/src/overlay_source_document.rs
- crates/brain-brew-formats/tests/csv_sparse_field_sources.rs
- crates/brain-brew-formats/tests/fixtures/csv_sparse_fields/after.yaml
- crates/brain-brew-formats/tests/fixtures/csv_sparse_fields/base.yaml
- crates/brain-brew-formats/tests/fixtures/csv_sparse_fields/before.yaml
- crates/brain-brew-formats/tests/fixtures/csv_sparse_fields/descriptor.yaml
- crates/brain-brew-formats/tests/fixtures/csv_sparse_fields/regions.csv
- documentation/docs/authoring/workspace.md
- .frontloop/composable-csv-authoring/done/0100-allow-sparse-overlay-field-values-to-come-from-csv.md
