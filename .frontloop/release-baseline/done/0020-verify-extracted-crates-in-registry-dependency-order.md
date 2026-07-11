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


## Completion Summary

- Added real cargo-package archive verification for core, formats, and CLI in publication order
- Validated extracted archive README/license material and packaged relative links
- Stripped/rejected internal path-dependency leakage and compiled only extracted artifacts against checksum-verified staged Cargo directory sources
- Added a real changed-core interface regression proving dependent extracted builds fail rather than accepting alpha.1-like mismatch
- Added separate pre-publish staged-artifact coherence and indexed-registry readiness modes; blocked states are nonzero rather than skipped success
- Integrated the pre-publish gate before every publish path and retained manual --yes publication requirement
- Documented alpha.2 manual core→index→formats→index→CLI sequencing and evidence boundaries
- Passed extracted gate/fixture tests, full Rust tests, metadata/fmt/clippy/docs/release smoke/dist plan, and Claude judgment

### Files Changed

- scripts/verify_extracted_crates.py
- scripts/tests/test_verify_extracted_crates.py
- scripts/publish_crates.sh
- scripts/check_cratesio_metadata.py
- devenv.nix
- .github/workflows/package-smoke.yml
- documentation/docs/reference/releasing.md
- crates/brain-brew-core/Cargo.toml
- crates/brain-brew-core/README.md
- crates/brain-brew-core/LICENSE
- crates/brain-brew-formats/Cargo.toml
- crates/brain-brew-formats/README.md
- crates/brain-brew-formats/LICENSE
- crates/brain-brew-cli/Cargo.toml
- crates/brain-brew-cli/README.md
- crates/brain-brew-cli/LICENSE
