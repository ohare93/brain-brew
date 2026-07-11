---
title: Choose Workbench trust and transaction scope
priority: critical
---

## Goal

Define the supported browser/workspace threat model and whether cross-root or cross-filesystem files may participate in one Apply.

## Acceptance Criteria

- State whether untrusted workspaces and foreign browser origins are in scope
- Choose capability-token, Host/Origin, CSP, and rendered-content policy
- Define whether external include roots or cross-filesystem batches are rejected or recoverably supported
- Document the maturity of the Workbench interface as stable or experimental

## Implementation Notes

Immediate read-only restriction can proceed before this decision.

## Questions

### Q1: Recommended: treat workspaces/content as untrusted, require a per-process capability plus Host/Origin checks and CSP, and reject Apply batches that cannot use the recoverable transaction contract. Approve this scope?
