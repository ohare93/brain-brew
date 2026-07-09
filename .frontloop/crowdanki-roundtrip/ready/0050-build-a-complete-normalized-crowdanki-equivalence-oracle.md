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
