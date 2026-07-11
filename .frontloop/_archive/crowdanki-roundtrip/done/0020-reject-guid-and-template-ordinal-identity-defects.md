---
title: Reject GUID and template ordinal identity defects
priority: critical
---

## Goal

Fail closed on duplicate/effectively colliding note GUIDs and non-unique, gapped, or invalid template ordinals rather than normalizing identity silently.

## Acceptance Criteria

- Duplicate GUIDs fail with all affected note indices
- GUID normalization/effective-collision rules are documented and tested
- Template ordinals must satisfy the supported uniqueness/contiguity model
- Import/export cannot silently reorder or renumber malformed templates
- Regression tests cover duplicate GUID and `[99,1,2,3]` probes

## Implementation Notes

Land before reviewable import plan so its identity report is reliable.


## Completion Summary

- Added one shared CrowdAnki identity gateway used by import, export, and round-trip projection
- Defined CrowdAnki GUIDs as opaque exact UTF-8 identifiers; rejected empty and duplicate effective GUIDs with every affected source/note index
- Made missing canonical GUID fallback to stable ID explicit and validated before GUID-assisted suggestion generation
- Enforced zero-based template invariant tmpls[index].ord == index with no sorting, renumbering, or silent repair
- Added malformed ordinal diagnostics for duplicate, gapped, reordered, negative, and overflow values including source paths and expected/found values
- Updated parity comparison so reordered template ordinals are identity defects rather than normalized away
- Added codec and CLI regressions for duplicate GUIDs, opaque lookalikes, and [99,1,2,3] plus other malformed ordinal probes
- Passed full default workspace tests, targeted codec/CLI tests, fmt, clippy, docs, release smoke, and Claude judgment

### Files Changed

- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- crates/brain-brew-cli/tests/cli.rs
- documentation/docs/authoring/importing-crowdanki.md
- documentation/docs/reference/cli.md
