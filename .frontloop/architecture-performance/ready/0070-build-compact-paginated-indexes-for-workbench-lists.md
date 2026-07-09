---
title: Build compact paginated indexes for Workbench lists
priority: low
---

## Goal

Avoid constructing full note/card/source detail collections before slicing and provide stable cursors/indexes for large decks.

## Acceptance Criteria

- List endpoints query compact cached indexes and materialize only requested rows
- Pagination order/cursors are deterministic across refreshes
- Limit 1 is measurably cheaper than limit 50 and full detail
- Invalidation follows complete source fingerprints
- Existing row and new item-51 E2E behavior remains correct

## Implementation Notes

After Workbench detail interfaces and cache correctness.
