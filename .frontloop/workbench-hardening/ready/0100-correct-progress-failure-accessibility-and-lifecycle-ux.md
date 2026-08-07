---
title: Correct progress failure accessibility and lifecycle UX
priority: medium
---

## Goal

Make deck-wide progress accurate and provide deterministic busy/error/status semantics, keyboard accessibility, and graceful shutdown/restart behavior.

## Acceptance Criteria

- Selected-note DOM counts cannot overwrite deck-wide progress
- Loading/busy/live/error state is announced semantically
- Interactive controls have labels, active-row state, and keyboard coverage
- Failure and local-storage error paths have native/unit and browser tests
- Server shutdown drains/rejects writes safely and restart surfaces transaction/draft recovery

## Implementation Notes

Can follow complete detail/draft workflows.
