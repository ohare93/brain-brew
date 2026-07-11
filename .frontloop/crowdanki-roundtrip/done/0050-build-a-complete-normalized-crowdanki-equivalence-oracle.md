---
title: Build a complete normalized CrowdAnki equivalence oracle
priority: critical
---

## Goal

Compare canonical and CrowdAnki states across every meaningful deck, schema, template, configuration, identity, note, tag, and media property.

## Acceptance Criteria

- Oracle builds on complete core semantic diff
- Normalization rules are explicit and narrowly scoped
- Single-property mutation tests prove every supported property is observed
- Unsupported CrowdAnki state remains fail-closed
- Import→export and export→import laws cover Unicode identities and structured media/messages

## Implementation Notes

Depends on core semantic-diff completion; required before UG goldens.


## Completion Summary

- Added a typed fail-closed normalized CrowdAnki equivalence oracle built on the compiler-exhaustive core semantic diff
- Defined six explicit symmetric round-trip loss dimensions and prohibited normalization of retained semantics
- Added a property-mutation matrix covering every CrowdAnki-representable property that survives projection
- Rejected unknown and non-default unsupported CrowdAnki state with typed category and canonical/CrowdAnki source paths
- Added import/export law coverage for NFC/CJK/RTL identities, GUID versus stable-ID behavior, structured messages, images, and declared media
- Made media equivalence proof explicit: byte handoff verifies hashes/bytes; reference-only returns NotProven and cannot prove media identity
- Documented oracle scope, round-trip losses, unsupported state, and release-proof requirements
- Passed focused/default tests, fmt, clippy, docs, release smoke, and Claude judgment

### Files Changed

- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- documentation/docs/reference/crowdanki-equivalence-oracle.md
- documentation/docs/reference/release-oracle.md
- documentation/sidebars.js
