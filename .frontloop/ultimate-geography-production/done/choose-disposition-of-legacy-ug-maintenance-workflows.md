---
title: Choose disposition of legacy UG maintenance workflows
priority: medium
---

## Goal

Decide separately whether Anki-to-source reconciliation, deleted-flag similarity analysis, and ZIP release packaging are restored or formally deprecated with replacements.

## Acceptance Criteria

- Record restore/deprecate decision for all three workflows
- Name the replacement or implementation owner for each
- Update the definition of fully migrated UG accordingly
- Remove stale workflow promises after the decision

## Implementation Notes

Can resolve while core production migration proceeds.

## Questions

### Q1: Recommended: restore deterministic declared-media ZIP packaging, defer Anki reconciliation to default/7500, and formally deprecate flag similarity unless an active maintainer need is demonstrated. Approve or revise.
