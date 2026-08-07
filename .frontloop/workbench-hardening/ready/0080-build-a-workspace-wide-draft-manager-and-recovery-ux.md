---
title: Build a workspace-wide draft manager and recovery UX
priority: high
---

## Goal

Ensure all staged scopes remain visible/applicable across unmounted languages and targets, with explicit discard, conflict, and restart recovery.

## Acceptance Criteria

- Draft inventory is independent of currently mounted DOM/view prefixes
- Apply includes exactly the user-selected workspace-wide drafts
- Users can inspect, discard one, discard all, export, and recover drafts
- Storage failures and schema upgrades produce actionable UX
- E2E covers unmounted scopes, restart, stale conflict, and recovery

## Implementation Notes

After typed contract; final Apply path depends on preview/CAS.
