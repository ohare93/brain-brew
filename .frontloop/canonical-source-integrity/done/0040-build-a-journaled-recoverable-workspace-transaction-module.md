---
title: Build a journaled recoverable workspace transaction module
priority: critical
---

## Goal

Replace misleading sequential-renames atomicity with plan, validate, commit, rollback, and restart recovery for multi-file workspace writes.

## Acceptance Criteria

- All target files and expected fingerprints are validated before mutation
- Commit records a journal and recoverable backups before the first replacement
- Injected failures at every commit step either roll back or leave an explicit recoverable state
- Cross-filesystem and external-include cases are rejected or handled according to a documented contract
- Crash/restart recovery tests prove no silently mixed old/new workspace

## Implementation Notes

May be developed beside source-document modules; filesystem implementation belongs in CLI.


## Completion Summary

- Added a CLI-owned typed workspace transaction plan with canonical-root, target-type, duplicate, expected fingerprint/existence, and same-filesystem validation
- Persisted durable journals, staged replacements, complete backups, commit progress, rollback state, and cooperative locking before replacements
- Added deterministic failure injection across prepare, backup, replace, finalize, rollback, and restart recovery
- Rejected external-root, cross-filesystem, symlink-escape, unsupported, and conflicting target plans under a documented fail-closed contract
- Fixed judge-found stale rollback restore-stage recovery wedge and proved attacker-modified derived stages are discarded and regenerated from verified backups
- Documented durability, platform, recovery, and migration contracts; left existing mutators unmigrated for tasks 0050/0060
- Passed focused transaction tests, full fmt/test/clippy, parent full CI, and independent Claude re-judgment

### Files Changed

- Cargo.lock
- crates/brain-brew-cli/Cargo.toml
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/workspace_transaction.rs
- documentation/docs/reference/workspace-transactions.md
- documentation/sidebars.js
