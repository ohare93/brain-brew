---
title: Generate collision-resistant Unicode-safe imported stable IDs
priority: critical
---

## Goal

Replace ASCII-only first-field slugging with deterministic Unicode-aware suggestions and explicit collision handling.

## Acceptance Criteria

- Non-Latin notes do not collapse to `note.unnamed`
- Suggestions are deterministic across platforms and locale settings
- Collisions receive stable disambiguation or require explicit override
- Diagnostics never advertise a nonexistent override workflow
- Tests cover Cyrillic, CJK, RTL, repeated first fields, blanks, and normalization equivalents

## Implementation Notes

First CrowdAnki task; preserve source GUIDs independently of suggested stable IDs.
