---
title: Verify extracted crates in registry dependency order
priority: critical
---

## Goal

Replace path-only confidence with a release gate that packages and verifies the exact `.crate` artifacts in core → formats → CLI order against the intended dependency versions.

## Acceptance Criteria

- The gate builds extracted crate contents rather than workspace paths
- Core, formats, and CLI are verified in publication order
- The alpha.1 published-core mismatch is covered by a regression test or scripted fixture
- Package archives contain required README and license material
- The gate fails before any upload when an internal interface/version is inconsistent

## Implementation Notes

Run after the version synchronization task; integrate with scripts/publish_crates.sh.
