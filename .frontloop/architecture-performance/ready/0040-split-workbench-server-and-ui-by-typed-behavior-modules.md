---
title: Split Workbench server and UI by typed behavior modules
priority: medium
---

## Goal

Decompose server routes/contracts/cache/apply/source/media/language and UI api/state/views/staging/preview implementations without exposing shallow pass-through interfaces.

## Acceptance Criteria

- Modules align with the versioned Workbench contract and ownership of behavior
- Apply safety and cache invariants each have one implementation location
- UI state and views use typed data rather than raw JSON indexing
- Deletion test confirms removed modules would redistribute meaningful complexity
- Embedded and browser tests remain green

## Implementation Notes

After Workbench hardening behavior stabilizes; do not repeat the completed Leptos migration.
