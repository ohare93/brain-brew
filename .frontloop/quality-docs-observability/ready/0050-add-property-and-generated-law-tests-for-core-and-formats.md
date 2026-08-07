---
title: Add property and generated law tests for core and formats
priority: high
---

## Goal

Cover invariant combinations beyond hand-picked examples for composition, translation commands, canonical round trips, semantic diff, and stable ordering.

## Acceptance Criteria

- Generated tests exercise intent/value/entity matrices
- Canonical decode-encode-decode and format-idempotence laws include hostile scalars and maps
- Translation command sequences preserve cross-map invariants
- Semantic diff mutation laws observe every property
- Seeds and minimized regressions are deterministic in CI

## Implementation Notes

Add after the corresponding corrected interfaces stabilize; individual behavior tasks still follow red-green-refactor.
