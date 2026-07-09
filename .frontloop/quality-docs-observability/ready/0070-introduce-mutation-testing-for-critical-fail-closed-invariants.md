---
title: Introduce mutation testing for critical fail-closed invariants
priority: medium
---

## Goal

Measure whether tests detect removed duplicate checks, containment checks, expected-base comparisons, CAS checks, media verification, and golden enforcement.

## Acceptance Criteria

- A bounded mutation configuration targets critical modules first
- Named mutations for each release-blocking invariant are killed
- Surviving mutants produce follow-up tests or documented exclusions
- Runtime is suitable for scheduled CI and focused local use
- A baseline score and non-regression threshold are recorded

## Implementation Notes

After critical regression suites land.
