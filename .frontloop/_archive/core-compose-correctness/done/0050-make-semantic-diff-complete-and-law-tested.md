---
title: Make semantic diff complete and law-tested
priority: critical
---

## Goal

Turn semantic diff into a trustworthy equivalence oracle covering every canonical property used by round-trip and parity tests.

## Acceptance Criteria

- Deck identity, adapter IDs, structured messages/images, ordering-sensitive schema, configuration, and all canonical fields participate
- Each single-property mutation is detected by a generated test matrix
- Equivalent normalized forms compare equal only where explicitly documented
- CrowdAnki and UG parity tests use the completed oracle
- Diff output remains deterministic and path-addressed

## Implementation Notes

Depends on semantic field values; should land before new goldens rely on it.


## Completion Summary

- Moved exact canonical semantic diff into a dedicated compiler-auditable module with exhaustive no-wildcard struct and enum traversal
- Covered every current deck, note-type, ordered field/template, note, FieldValue/message/image, media, adapter-ID, and typed tombstone provenance property
- Added deterministic structured before/after changes with stable path sorting and injective semantic paths
- Added a 40-case single-property mutation inventory, all 22 tombstone address variants, and identity/determinism/inversion/order/representation/equality laws
- Defined crowdanki-export-import-v1 as an explicit six-loss owned projection while keeping the exact oracle uncompromised
- Migrated CrowdAnki, CLI UG-style, all 74 main targets, all 26 companion targets, and optional release-oracle parity to projection plus exact semantic diff or explicit collision-loss reporting
- Removed the legacy incomplete diff and old CrowdAnki spot-check comparator
- Passed 558 CPU-bounded non-E2E tests, core/CrowdAnki/UG suites, clippy, 13 E2E tests, docs, release smoke, and Claude judgment

### Files Changed

- crates/brain-brew-core/src/semantic_diff.rs
- crates/brain-brew-core/src/compose.rs
- crates/brain-brew-core/src/lib.rs
- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/tests.rs
- crates/brain-brew-core/src/validate.rs
- crates/brain-brew-core/tests/semantic_diff_laws.rs
- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
- crates/brain-brew-cli/tests/ug_style_fixture.rs
- documentation/docs/authoring/diff-explain.md
