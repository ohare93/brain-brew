---
title: Make blank deletion and target adaptations explicit and general
priority: high
---

## Goal

Implement the approved blank policy and replace UG-specific magic-reason `target_additions` with a documented typed adaptation model.

## Acceptance Criteria

- Blank direct entries cannot silently erase global content
- Explicit deletions/adaptations are path-scoped, reviewed, and represented in coverage
- The magic UG reason string is removed from format semantics
- Schema and migration docs explain the general adaptation model
- Round-trip tests cover legacy UG data and canonical new emission

## Implementation Notes

Depends on blank-policy decision and strict YAML union work.
