---
title: Add bounded secure network fetch policy
priority: high
---

## Goal

Constrain lock updates and fetches by transport, redirect, timeout, byte, and decompression budgets.

## Acceptance Criteria

- HTTPS is required except explicit local/test adapters
- Connect/read/total timeouts are configured and tested
- Redirect count and cross-scheme behavior are bounded
- Download and extracted-byte/file-count/ratio limits prevent archive bombs
- Failures clean temporary state and report the exhausted budget

## Implementation Notes

Implement after source-specific lock schemas define fetch adapters.


## Completion Summary

- Added one injectable FetchPolicy used by GitHub API, codeload, and tarball package sources
- Required HTTPS in production, disabled automatic redirects, bounded manual redirects, rejected downgrade/credentials/unsupported schemes, and stripped auth across hosts
- Added connect/read/total monotonic deadlines and bounded streaming temporary files with Content-Length and chunked overflow enforcement
- Added compressed, JSON, decompressed tar, per-file, entry-count, expanded-byte, path, metadata, and expansion-ratio limits
- Added structured source/budget/current/limit failures with temporary cleanup and valid-cache preservation
- Added deterministic local transport, redirect, timeout, body framing, archive bomb, truncation, and cleanup tests and documented defaults/rationale
- Passed full fmt/test/clippy, E2E, release smoke, focused lock/fetch tests, and independent Claude judgment

### Files Changed

- Cargo.lock
- crates/brain-brew-cli/Cargo.toml
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/fetch_policy.rs
- crates/brain-brew-cli/src/package_tree.rs
- crates/brain-brew-cli/src/commands/lock.rs
- crates/brain-brew-formats/src/lockfile.rs
- crates/brain-brew-formats/tests/lockfile_yaml.rs
- documentation/docs/authoring/packages-locking.md
- documentation/docs/reference/lockfile.md
