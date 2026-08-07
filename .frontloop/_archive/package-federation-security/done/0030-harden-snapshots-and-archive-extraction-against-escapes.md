---
title: Harden snapshots and archive extraction against escapes
priority: critical
---

## Goal

Ensure copied, cloned, and extracted packages cannot retain symlink, special-file, traversal, or root-escape content outside the hashed tree.

## Acceptance Criteria

- The approved symlink policy is enforced for all source kinds
- Archive traversal, absolute paths, hard links, devices, and special files are rejected
- The hashed tree is exactly the tree later used by planning
- Existing out-of-tree manifest/symlink reproductions fail
- Cache writes remain content-addressed and recover safely after rejection

## Implementation Notes

Depends on safe paths and symlink-policy decision.


## Completion Summary

- Adopted and documented a reject-all-symlinks policy for path, Git, tarball, staging, and cached package trees
- Added centralized package-tree validation/copy/extraction with no-follow/create-new semantics, normalized permissions, and special-file rejection
- Rejected traversal/platform paths, symlink/hard-link/device/FIFO/socket/sparse/unknown entries, PAX sparse/path tricks, and duplicate/colliding targets before writes
- Validated trees before and after hashing and consumed exactly the validated content-addressed tree
- Added locked same-filesystem atomic cache publication, failure cleanup, and warm-cache policy/hash revalidation
- Added adversarial archive/source/cache tests and preserved valid deterministic package workflows
- Passed full fmt/test/clippy, E2E, release smoke, and independent Claude judgment

### Files Changed

- Cargo.lock
- crates/brain-brew-cli/Cargo.toml
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/package_tree.rs
- crates/brain-brew-cli/src/commands/lock.rs
- crates/brain-brew-cli/tests/lock_cli.rs
- documentation/docs/authoring/packages-locking.md
- documentation/docs/reference/lockfile.md
