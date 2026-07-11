---
title: Reject duplicate keys in every dynamic YAML map
priority: critical
---

## Goal

Make canonical deck, overlay, translation, manifest, lock, and media-map decoders reject duplicate IDs or fields before any value is overwritten.

## Acceptance Criteria

- Duplicate keys fail at all dynamic map levels identified by the audit
- Errors name the source file, schema path, and duplicate key where available
- `fmt` and every mutator leave bytes unchanged on duplicate-key failure
- Regression tests cover overlays, contextual translations, manifests, locks, media maps, notes, and targets
- Canonical positive round trips remain byte-stable

## Implementation Notes

First implementation task in this epic; use TDD against the reproduced data-loss probes.


## Completion Summary

- Added recursive fail-closed duplicate-key detection before serde map deserialization
- Wired strict duplicate validation through canonical decks, overlays/translations, manifests, locks, media maps, includes, CLI mutators, verify, and Workbench
- Added source/schema/key diagnostics and byte-unchanged failure regressions
- Added focused coverage for notes, targets, contextual translations, overlays, locks, manifests, media maps, and canonical round trips
- Passed delegated fmt, 413-test non-browser suite, clippy, and independent Claude judgment

### Files Changed

- crates/brain-brew-formats/src/strict_yaml.rs
- crates/brain-brew-formats/src/canonical_yaml.rs
- crates/brain-brew-formats/src/manifest.rs
- crates/brain-brew-formats/src/lockfile.rs
- crates/brain-brew-formats/src/media_map.rs
- crates/brain-brew-formats/src/source_includes.rs
- crates/brain-brew-formats/src/lib.rs
- crates/brain-brew-formats/tests/canonical_yaml.rs
- crates/brain-brew-formats/tests/overlay_yaml.rs
- crates/brain-brew-formats/tests/manifest_yaml.rs
- crates/brain-brew-formats/tests/lockfile_yaml.rs
- crates/brain-brew-formats/tests/media_map.rs
- crates/brain-brew-cli/src/io.rs
- crates/brain-brew-cli/src/commands/lock.rs
- crates/brain-brew-cli/src/commands/media.rs
- crates/brain-brew-cli/src/commands/verify.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/tests/cli.rs
