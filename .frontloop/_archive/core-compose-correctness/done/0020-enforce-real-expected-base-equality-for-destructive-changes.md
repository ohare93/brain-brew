---
title: Enforce real expected-base equality for destructive changes
priority: critical
---

## Goal

Apply the approved precondition model to complete and media replacements, with deterministic mismatch errors and no presence-only escape.

## Acceptance Criteria

- Note, note-type, field, template, and media complete replacements compare the actual prior entity/value
- Wrong-but-present baselines fail before mutation
- Override intent cannot silently bypass stale-overlay protection
- Diagnostics identify path, expected precondition, and actual state without requiring English parsing
- A mutation matrix covers matching, missing, stale, and concurrent overlay cases

## Implementation Notes

Depends on semantic field values and the replacement-precondition decision.


## Completion Summary

- Added canonical SHA-256 v1 EntityFingerprint with algorithm/domain/entity-kind/version separation and strict canonical parsing
- Applied actual-current entity fingerprints to complete note, note-type, field-definition, card-template, and media replace/override/remove preconditions
- Preserved exact typed expected values for sparse changes and rejected presence-only legacy baselines with migration guidance
- Added structured compose/explain diagnostics carrying path, entity kind, intent, overlay, expected, actual, code, and category
- Made diff --as-overlay generate prior fingerprints automatically and fail closed for unsupported ordering
- Added golden vectors, per-semantic-property mutation tests, complete mutation/precondition matrix, ordered concurrent-overlay tests, YAML migration tests, and UG-style fixture migration
- Documented ADR-0019 and the canonical entity fingerprint specification
- Passed full CPU-bounded tests, clippy, docs, release smoke, controlled 13/13 baseline and payload E2E reruns, and final Claude ACCEPT

### Files Changed

- CHANGELOG.md
- Cargo.toml
- Cargo.lock
- crates/brain-brew-core/Cargo.toml
- crates/brain-brew-core/src/fingerprint.rs
- crates/brain-brew-core/src/lib.rs
- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/compose.rs
- crates/brain-brew-core/tests/entity_fingerprint.rs
- crates/brain-brew-core/tests/overlay_compose.rs
- crates/brain-brew-formats/src/canonical_yaml.rs
- crates/brain-brew-formats/tests/overlay_yaml.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
- crates/brain-brew-cli/src/overlay_draft.rs
- crates/brain-brew-cli/src/commands/explain.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-cli/tests/federated_media_ownership.rs
- fixtures/ug-style/tombstone-australia.yaml
- documentation/docs/reference/entity-fingerprints.md
- documentation/docs/reference/yaml.md
- documentation/docs/concepts/overlays.md
- documentation/docs/authoring/diff-explain.md
- documentation/docs/reference/decisions/0007-require-explicit-conflict-and-destructive-change-semantics.md
- documentation/docs/reference/decisions/0018-model-note-fields-as-semantic-values.md
- documentation/docs/reference/decisions/0019-use-canonical-entity-fingerprints-for-complete-destructive-changes.md
- documentation/docs/reference/decisions/README.md
