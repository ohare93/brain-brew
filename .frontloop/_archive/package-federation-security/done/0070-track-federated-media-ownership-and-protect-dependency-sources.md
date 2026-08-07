---
title: Track federated media ownership and protect dependency sources
priority: critical
---

## Goal

Carry package-specific media-root provenance through planning, verification, export, and mutation so root commands cannot rewrite locked dependency caches.

## Acceptance Criteria

- Every media declaration/reference is attributable to a package and authorized media root
- Verification resolves bytes against the owning package root
- Mutation commands only propose root-workspace files unless an explicit supported vendor workflow exists
- Locked/cache sources are read-only and integrity-checked after operations
- Cross-package media collision and missing-root tests are deterministic

## Implementation Notes

Depends on provenance-aware planner and safe mutation modules.


## Completion Summary

- Added final per-target media ownership plans by replaying ordered overlay declarations and preserving package/source/root/document provenance
- Bound every media ID/path reference to its final declaration owner and rejected cross-package ID/path/output ambiguity
- Added repeatable package-qualified media-root mappings while retaining unqualified root-package-only compatibility
- Migrated manifest verify, export, and Workbench media reads/copies to owner-authorized roots with no target-root fallback
- Guarded media hash/images-to-refs to root-workspace-owned sources/declarations before writes and moved them onto typed source documents and recoverable transactions
- Validated locked package trees before and after mutation without cache repair and added collision/root/ownership/integrity regressions
- Passed full fmt/test/clippy, 13 E2E, 74+26 fixture verification, release smoke, and independent Claude judgment

### Files Changed

- crates/brain-brew-cli/src/media_ownership.rs
- crates/brain-brew-cli/src/media_assets.rs
- crates/brain-brew-cli/src/planner.rs
- crates/brain-brew-cli/src/io.rs
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/args.rs
- crates/brain-brew-cli/src/help.rs
- crates/brain-brew-cli/src/commands/media.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/src/commands/export.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/src/commands/lock.rs
- crates/brain-brew-cli/tests/federated_media_ownership.rs
- documentation/docs/authoring/media.md
- documentation/docs/authoring/packages-locking.md
- documentation/docs/authoring/verify-export.md
- documentation/docs/reference/workbench.md
