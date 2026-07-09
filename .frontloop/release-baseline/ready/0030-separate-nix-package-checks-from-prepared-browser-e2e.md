---
title: Separate Nix package checks from prepared browser E2E
priority: critical
---

## Goal

Make `nix build` produce and test the CLI deterministically without invoking an unprepared WebDriver harness, while retaining a distinct prepared browser gate.

## Acceptance Criteria

- The Nix package check builds the CLI and non-browser tests successfully
- Browser E2E runs only in a derivation/job that provisions the CLI, UI assets, browser, and driver
- `nix build .#checks.x86_64-linux.brainbrew` passes
- CI retains all 13 browser scenarios in the prepared gate

## Implementation Notes

Can proceed in parallel with extracted-crate verification after version synchronization.
