---
title: Model scalar image and message fields as semantic values
priority: critical
---

## Goal

Remove empty-string placeholders so composition, blank checks, fills, overrides, validation, and differences recognize every field representation.

## Acceptance Criteria

- Core exposes one semantic field-value abstraction covering scalar, structured image, and structured message content
- Add/merge/fill cannot erase an existing structured value by treating it as blank
- Unknown structured-media references fail validation
- Rendering and CrowdAnki lowering remain deterministic
- Regression tests cover every intent against each value representation

## Implementation Notes

First implementation task; prerequisite for reliable expected-base and semantic-diff work.


## Completion Summary

- Introduced one pure core FieldValue/FieldMap abstraction for scalar, structured image, and structured message note fields
- Removed split scalar/image/message maps and empty-string placeholder states across core, formats, CLI, Workbench, media, and adapters
- Made blank/fill/add/merge/replace/override/expected-base behavior representation-aware with exhaustive intent matrix coverage
- Moved unknown structured media, malformed value, message reference, and message cycle checks into canonical validation with note/field paths
- Updated translation, message resolution, rendering, YAML/source documents, CrowdAnki import/export, Workbench staging, media scanning, and semantic differences
- Added structured expected-base YAML and ADR-0018 while preserving canonical syntax and UG/CrowdAnki parity
- Passed CPU-bounded full tests, clippy, 74+26 parity, E2E, docs, release smoke, and Claude judgment

### Files Changed

- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/compose.rs
- crates/brain-brew-core/src/validate.rs
- crates/brain-brew-core/src/messages.rs
- crates/brain-brew-core/src/translation.rs
- crates/brain-brew-core/tests/canonical_deck_validation.rs
- crates/brain-brew-core/tests/overlay_compose.rs
- crates/brain-brew-core/tests/semantic_diff.rs
- crates/brain-brew-core/tests/translation_coverage.rs
- crates/brain-brew-formats/src/canonical_yaml.rs
- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/src/media.rs
- crates/brain-brew-formats/src/source_document.rs
- crates/brain-brew-formats/src/source_includes.rs
- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/src/overlay_source_document.rs
- crates/brain-brew-formats/tests/canonical_yaml.rs
- crates/brain-brew-formats/tests/overlay_yaml.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- crates/brain-brew-formats/tests/media_references.rs
- crates/brain-brew-formats/tests/emitter_roundtrip.rs
- crates/brain-brew-formats/tests/yaml_scalar_adversarial.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
- crates/brain-brew-cli/src/planner.rs
- crates/brain-brew-cli/src/overlay_draft.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/tests/cli.rs
- documentation/docs/reference/decisions/0018-model-note-fields-as-semantic-values.md
- documentation/docs/reference/decisions/0016-use-structured-image-field-references-and-severable-media-includes.md
- documentation/docs/reference/decisions/README.md
