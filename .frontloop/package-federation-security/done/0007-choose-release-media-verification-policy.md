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

## Decision

Adopt strict-by-default media release verification. Any media-bearing verify or manifest export requires explicit owner roots, canonical non-empty SHA-256 hashes, present matching bytes, and complete reference/collision validation. `--media-mode reference-only` is an explicit development/structural mode that still validates references but reports `NOT RELEASE-READY` and `release_ready: false`; a missing root never downgrades implicitly. Package-qualified roots use the ownership mapping introduced by task 0070. Hashless fixtures invoke reference-only explicitly and are documented as structural evidence only.

Agent Tick was unavailable during execution, so the orchestrator applied the task's conservative recommended policy while fulfilling the user's full remediation instruction.
