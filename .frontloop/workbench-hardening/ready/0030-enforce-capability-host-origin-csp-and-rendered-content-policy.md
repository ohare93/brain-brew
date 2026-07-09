---
title: Enforce capability Host Origin CSP and rendered-content policy
priority: critical
---

## Goal

Harden the loopback file-writing server and browser document according to the approved trust model.

## Acceptance Criteria

- Each process requires an unguessable capability for state-changing and sensitive routes
- Unexpected Host and Origin requests are rejected
- CSP prevents unexpected script/network execution
- Rendered deck HTML is sandboxed or sanitized according to policy rather than entering unrestricted innerHTML
- Request limits and CSRF-like cross-origin tests cover foreign sites

## Implementation Notes

Depends on trust-model decision and typed routing contract.
