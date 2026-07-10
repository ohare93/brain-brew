---
title: Mark unsafe Apply experimental or make release Workbench read-only
priority: critical
---

## Goal

Prevent stale/partial/canonicality-unsafe writes from being presented as a supported release workflow until the hardening sequence lands.

## Acceptance Criteria

- Release behavior is explicitly read-only or guarded by an unmistakable experimental opt-in
- Documentation and UI explain the current risk and retained staging/export options
- API tests prove writes are unavailable without the opt-in
- No existing verify/browse workflow regresses
- A removal condition references the CAS, canonical mutation, preview, and recovery tasks

## Implementation Notes

First Workbench task; intentionally a temporary containment measure.


## Completion Summary

- Made default and distributed Workbench builds read-only at the server layer
- Required both compile-time workbench-write-dev capability and explicit --enable-write for development writes, with no environment bypass
- Rejected Apply and new-language writes with HTTP 403 before workspace/cache/file side effects in default mode
- Exposed server write capability to the UI, added prominent read-only/development warnings, retained local drafts and browse/compare/media/preview
- Added direct-HTTP containment, feature/flag, UI host, and dual-mode browser coverage
- Documented known risks and exact CAS/preview/canonical-transaction/security removal conditions
- Rebuilt embedded assets and passed default/development tests, clippy, UI build/embed, 13 E2E, docs, release containment proof, and Claude judgment

### Files Changed

- crates/brain-brew-cli/Cargo.toml
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/src/help.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-workbench-ui/src/lib.rs
- crates/brain-brew-workbench-ui/static/workbench.css
- crates/brain-brew-workbench-ui/tests/workspace_summary.rs
- crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs
- crates/brain-brew-workbench-e2e/README.md
- crates/brain-brew-cli/assets/workbench/index.html
- crates/brain-brew-cli/assets/workbench/brain_brew_workbench_ui-202dc73fcf491da9.js
- crates/brain-brew-cli/assets/workbench/brain_brew_workbench_ui-202dc73fcf491da9_bg.wasm
- crates/brain-brew-cli/assets/workbench/workbench-66e33cced77ddc3f.css
- devenv.nix
- documentation/docs/reference/cli.md
- documentation/docs/reference/workbench.md
