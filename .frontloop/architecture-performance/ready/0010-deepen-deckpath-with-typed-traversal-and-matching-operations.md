---
title: Deepen DeckPath with typed traversal and matching operations
priority: medium
---

## Goal

Remove remaining CLI string parsing by exposing typed parent, entity, field, descendant, and pattern operations at the core interface.

## Acceptance Criteria

- Translation and Workbench callers no longer parse DeckPath strings manually
- Typed operations cover parent/entity/field access and descendant matching
- Stable display/serialization remains compatible or migrates explicitly
- Property tests cover parse/display/traversal laws
- No filesystem authorization concerns leak into core DeckPath

## Implementation Notes

Start after core path/error shapes stabilize.
