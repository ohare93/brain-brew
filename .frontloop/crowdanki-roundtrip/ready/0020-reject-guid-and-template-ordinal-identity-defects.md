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
