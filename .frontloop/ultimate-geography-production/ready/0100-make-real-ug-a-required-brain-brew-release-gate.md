---
title: Make real UG a required Brain Brew release gate
priority: critical
---

## Goal

Run pinned production UG format, all-target verify with media, representative export, Workbench smoke, and independent goldens before Brain Brew or UG publication.

## Acceptance Criteria

- The gate checks the reconciled immutable UG revision
- All 74 main and 26 Hardcore targets verify with media bytes
- Representative outputs pass mandatory goldens and archive validation
- Workbench language selection uses production catalog data
- Brain Brew release workflow and UG CI both fail on consumer regression

## Implementation Notes

Final UG acceptance task after all production blockers; integrate into release-baseline reusable workflow.
