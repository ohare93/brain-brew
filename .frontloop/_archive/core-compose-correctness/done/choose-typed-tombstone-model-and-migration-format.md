---
title: Choose typed tombstone model and migration format
priority: high
---

## Goal

Define path- or entity-typed tombstones so removals cannot alias unrelated stable IDs and all removable entity kinds receive explicit reuse protection.

## Acceptance Criteria

- Choose typed entity variants or DeckPath-addressed tombstones
- Cover notes, note types, fields, templates, media, and future entity kinds
- Define YAML compatibility/migration from the flat stable-ID set
- Specify validation and diagnostic behavior for reuse attempts

## Implementation Notes

Blocks typed tombstone implementation but not semantic field-value work.

## Questions

### Q1: Recommended: use typed entity/path variants serialized explicitly, with a compatibility reader and canonical writer migration. Approve or choose an alternative.
