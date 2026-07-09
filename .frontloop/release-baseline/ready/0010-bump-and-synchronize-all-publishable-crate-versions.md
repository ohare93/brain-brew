---
title: Bump and synchronize all publishable crate versions
priority: critical
---

## Goal

Apply the approved preview version across workspace metadata, exact internal dependencies, lock data, changelog, and release configuration so no source claims the immutable alpha.1 interface.

## Acceptance Criteria

- All publishable crates and exact internal dependencies use the approved version
- Cargo.lock, changelog, dist configuration, and user-facing version references agree
- No stale alpha.1 source/interface claims remain outside historical records
- Workspace tests and metadata validation pass

## Design Decisions

- Keep publishable crates versioned in lockstep unless the release-policy task explicitly decides otherwise

## Implementation Notes

Prerequisite: clarify/choose-next-preview-version-and-supported-release-channels.
