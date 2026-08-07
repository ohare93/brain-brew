---
title: Establish a pinned reconciled UG consumer baseline
priority: critical
---

## Goal

Reconcile migration/upstream history and pin the exact Brain Brew source revision, UG revision, parity baseline, and command set used for acceptance.

## Acceptance Criteria

- Migration and upstream divergence is reconciled or explicitly documented with a stable base
- UG CI and contributor instructions use an immutable Brain Brew revision/version
- Parity inputs are repository-relative and immutable
- A baseline report records current target counts and known migration deltas
- The pinned tool builds through its documented installation path

## Implementation Notes

First production task; depends on release version/channel choice for final pin.
