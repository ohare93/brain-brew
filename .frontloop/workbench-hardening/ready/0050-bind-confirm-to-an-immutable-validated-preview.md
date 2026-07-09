---
title: Bind Confirm to an immutable validated preview
priority: critical
---

## Goal

Require a server-issued preview token that commits exactly the reviewed edit set against the reviewed fingerprints.

## Acceptance Criteria

- Preview validates canonical result, domain composition, policy, and complete file plan
- Server returns a short-lived single-use token bound to edits and fingerprints
- Confirm cannot alter or bypass the previewed payload
- Expired, reused, stale, or mismatched tokens fail without writes
- Browser E2E covers preview-conflict-confirm and cancellation

## Implementation Notes

After typed contracts and CAS; token is not a substitute for commit-time CAS.
