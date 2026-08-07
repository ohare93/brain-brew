---
title: Add cross-platform artifact and interruption smoke tests
priority: medium
---

## Goal

Exercise package install, path handling, rename/recovery behavior, and representative CLI flows on Linux, macOS, Windows, and the declared ARM policy.

## Acceptance Criteria

- Produced artifacts install and report the expected version on each supported platform
- Path and replacement tests cover Windows and cross-filesystem behavior
- Interrupted transaction recovery is exercised where runners permit
- Unsupported ARM targets are explicitly documented or tested
- Pre-tag workflow runs the supported platform matrix before publication

## Implementation Notes

Coordinate release-baseline and transaction modules.
