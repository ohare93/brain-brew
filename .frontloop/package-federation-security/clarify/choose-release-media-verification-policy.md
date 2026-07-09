---
title: Choose release media verification policy
priority: critical
---

## Goal

Decide whether any declared media makes a byte root mandatory for release verify/export and how manifests configure package-specific roots.

## Acceptance Criteria

- Define development versus release verification modes
- Specify when empty or malformed hashes are permitted
- Define manifest/CLI behavior for package-specific media roots
- Document export behavior when no byte root is available

## Implementation Notes

Blocks final media-integrity enforcement and UG release gate.

## Questions

### Q1: Recommended: allow reference-only checks as an explicit development mode, but require package-owned media roots, non-empty valid hashes, and byte validation for release verify/export. Approve this policy?
