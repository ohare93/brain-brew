---
title: Create a reviewable suggested-ID import plan
priority: high
---

## Goal

Replace blanket `--accept-suggested-ids` with an inspectable plan/override artifact supporting selective approval and repeatable imports.

## Acceptance Criteria

- A dry-run emits proposed note/note-type/template IDs with source GUID/model evidence
- Maintainers can override selected suggestions in a documented file
- Import refuses unresolved collisions and unreviewed changes according to policy
- Re-running with the same plan is deterministic
- CLI help/docs cover generate, review, apply, and recovery steps

## Implementation Notes

Depends on Unicode-safe suggestion and identity validation.
