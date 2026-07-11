---
title: Choose explicit blank translation semantics
priority: high
---

## Goal

Replace implicit global deletion from blank direct translations with an intentional reviewed category or rejection policy.

## Acceptance Criteria

- Choose reject, explicit deletion/adaptation intent, or warning category
- Define path scoping and coverage effects
- Specify migration for existing blank entries
- Document Workbench and CLI authoring behavior

## Implementation Notes

Blocks blank-value behavior implementation.

## Questions

### Q1: Recommended: reject blank direct translations and require a path-scoped explicit deletion/target-adaptation intent. Approve or choose a reviewed blank category?
