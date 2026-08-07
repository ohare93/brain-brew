---
title: Implement typed tombstones for every removable entity
priority: high
---

## Goal

Prevent removed identities from being reused across or within entity kinds and preserve removal provenance through composition.

## Acceptance Criteria

- The selected typed/path tombstone representation is implemented
- All removable entity kinds create and consult tombstones
- Legacy tombstones migrate or fail according to the approved policy
- Cross-kind identical stable IDs no longer alias
- Composition and YAML tests cover removal/reintroduction across overlay stacks

## Implementation Notes

Depends on the tombstone decision and strict YAML migration support.


## Completion Summary

- Added ordered typed TombstoneAddress variants with full parent scope, TombstoneRecord, and deterministic RemovalProvenance
- Made every current mutation address consult exact and ancestor tombstones before all intents and every remove operation record provenance without resurrection
- Prevented exact-address reuse while allowing identical StableIds across entity kinds, siblings, and different parent scopes
- Added strict flat legacy YAML inference only for exactly one retained top-level note/note-type/media identity and actionable failure for unknown, ambiguous, or nested IDs
- Made canonical YAML always emit explicit typed path records with stable provenance round-trips and strict kind/path validation
- Migrated validation, translation, media, CrowdAnki, semantic diff, CLI JSON/export, and Workbench active-entity behavior to typed blocking semantics
- Added comprehensive kind/parent/order/reuse/duplicate/provenance/migration tests and ADR-0020
- Passed focused/full constituent suites, UG 74+26 mutation regressions, clippy, E2E, docs, release smoke, and Claude judgment

### Files Changed

- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/compose.rs
- crates/brain-brew-core/src/validate.rs
- crates/brain-brew-core/src/translation.rs
- crates/brain-brew-core/src/tests.rs
- crates/brain-brew-core/tests/overlay_compose.rs
- crates/brain-brew-core/tests/canonical_deck_validation.rs
- crates/brain-brew-core/tests/content_validation.rs
- crates/brain-brew-core/tests/semantic_diff.rs
- crates/brain-brew-core/tests/translation_coverage.rs
- crates/brain-brew-formats/src/canonical_yaml.rs
- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/src/media.rs
- crates/brain-brew-formats/tests/canonical_yaml.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- crates/brain-brew-formats/tests/emitter_roundtrip.rs
- crates/brain-brew-formats/tests/media_references.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
- crates/brain-brew-formats/tests/yaml_scalar_adversarial.rs
- crates/brain-brew-cli/src/output.rs
- crates/brain-brew-cli/src/commands/explain.rs
- crates/brain-brew-cli/src/commands/export.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/tests/ug_style_fixture.rs
- documentation/docs/reference/decisions/0020-address-removals-with-typed-path-tombstones.md
- documentation/docs/reference/decisions/0007-require-explicit-conflict-and-destructive-change-semantics.md
- documentation/docs/reference/decisions/0019-use-canonical-entity-fingerprints-for-complete-destructive-changes.md
- documentation/docs/reference/decisions/README.md
- documentation/docs/reference/yaml.md
- documentation/docs/concepts/canonical-deck.md
- documentation/docs/reference/glossary.md
