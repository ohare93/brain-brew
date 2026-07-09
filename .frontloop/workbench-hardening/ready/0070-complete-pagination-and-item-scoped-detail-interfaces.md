---
title: Complete pagination and item-scoped detail interfaces
priority: high
---

## Goal

Make every item beyond row 50 reachable and complete ADR-015 list/detail behavior for notes, cards, source strings, metadata, and comparisons.

## Acceptance Criteria

- UI exposes deterministic paging or virtualization for all lists
- Card, source-string, and metadata detail routes return selected-item payloads
- Comparison is selected-item scoped rather than whole-deck
- List endpoints avoid constructing full detail payloads before slicing
- E2E reaches and edits item 51+ in every applicable view

## Implementation Notes

Can begin after typed contracts; retain default/0150 cross-language matrix as a related independent task.
