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


## Completion Summary

- Separated real Nix CLI package testing from browser WebDriver E2E by explicit Cargo package ownership
- Made `nix build .#checks.x86_64-linux.brainbrew` build the real CLI and test exactly non-browser packages
- Made `nix run .#brainbrew` execute the Nix-built artifact
- Added prepared Linux Devenv E2E script that builds fresh UI/assets and write-enabled CLI, provisions Chromium/chromedriver, and runs all 13 scenarios
- Made Nix package and prepared browser E2E required CI/release quality gates
- Added partition regression checks and documented platform/recovery boundaries
- Passed real Nix build/run smoke, 13 E2E scenarios, full tests, fmt/clippy, docs, release smoke, workflow/flake validation, and Claude judgment

### Files Changed

- flake.nix
- devenv.nix
- scripts/run_workbench_e2e.sh
- scripts/tests/test_nix_e2e_partition.py
- .github/workflows/ci.yml
- .github/workflows/release.yml
- crates/brain-brew-cli/tests/lock_cli.rs
- documentation/docs/getting-started/install.md
- documentation/docs/reference/releasing.md
- documentation/docs/reference/workbench.md
