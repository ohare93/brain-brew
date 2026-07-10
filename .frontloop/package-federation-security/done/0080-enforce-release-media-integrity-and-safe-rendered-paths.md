---
title: Enforce release media integrity and safe rendered paths
priority: critical
---

## Goal

Apply the approved media policy consistently to verify/export and reject unsafe or HTML-breaking media paths before rendering or copying.

## Acceptance Criteria

- Release verify/export require owned media roots, valid non-empty hashes, and byte validation when media is declared
- Development reference-only mode is explicit and cannot be mistaken for release readiness
- Export validates references even when byte copying is disabled
- Media paths are canonical, symlink-safe, and safely encoded or rejected for HTML attributes
- Dirty/stale output cannot survive failed export

## Implementation Notes

Depends on media-policy decision, ownership planning, and clean output transactions.


## Completion Summary

- Made media-bearing verify and manifest export strict by default with owner roots, canonical non-empty SHA-256, present bytes, and streamed hash comparison
- Added explicit --media-mode reference-only that retains semantic reference/collision checks and reports NOT RELEASE-READY/release_ready false
- Kept export clean-tree transactional publication and validated references even when bytes are intentionally omitted
- Added portable hostile media path restrictions, UTF-8 URL encoding, HTML attribute escaping, and scanner entity/percent normalization
- Added strict/reference-only, owner/root/hash/byte/mismatch/collision/hostile-render/output preservation regressions
- Marked all hashless UG fixture helpers and release smoke as explicit structural-only reference mode with drift tests
- Updated changelog, CLI/media/release/UG documentation and scripts
- Passed full CPU-bounded tests, clippy, 13 E2E, docs, release smoke, focused media tests, and Claude re-judgment

### Files Changed

- .sd/tasks.yaml
- CHANGELOG.md
- crates/brain-brew-cli/src/media_verification.rs
- crates/brain-brew-cli/src/media_assets.rs
- crates/brain-brew-cli/src/args.rs
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/help.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/src/commands/export.rs
- crates/brain-brew-cli/tests/release_media_integrity.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-cli/tests/cli_contract.rs
- crates/brain-brew-cli/tests/ug_style_fixture.rs
- crates/brain-brew-core/src/compose.rs
- crates/brain-brew-core/tests/overlay_compose.rs
- crates/brain-brew-formats/src/safe_relative_path.rs
- crates/brain-brew-formats/src/media.rs
- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/tests/safe_relative_path.rs
- crates/brain-brew-formats/tests/media_references.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- documentation/docs/authoring/media.md
- documentation/docs/authoring/verify-export.md
- documentation/docs/concepts/media.md
- documentation/docs/examples/ultimate-geography.md
- documentation/docs/reference/cli.md
- documentation/docs/reference/releasing.md
- scripts/release_smoke.sh
- scripts/sync-ug-fixture.sh
- scripts/ug-fixture-sync/README.md
