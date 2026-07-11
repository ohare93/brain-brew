---
title: Add media-byte handoff and safe import output transactions
priority: high
---

## Goal

Import declared media bytes alongside canonical declarations and prevent accidental overwrite or partial workspace creation.

## Acceptance Criteria

- Import inventories CrowdAnki media and verifies every referenced byte
- Generated declarations receive real hashes when bytes are available
- Output defaults to a new clean destination and requires explicit overwrite intent
- Source plus media commit through the workspace transaction module
- Missing/duplicate/unsafe media paths fail before any output changes

## Implementation Notes

Depends on source transactions and package-safe media paths.


## Completion Summary

- Added typed CrowdAnki media inventory with JSON source locations and content-reference cross-checking
- Added authorized media-root byte handoff with one-read-per-path provenance evidence and real SHA-256 declarations
- Rejected missing, duplicate, unused, unsafe, case-colliding, and stale media paths/evidence before any output publication
- Published deck source plus all media in a clean staged tree through journaled backup/rollback/recovery transaction semantics
- Made existing output refuse by default and force replacement remove stale media while rejecting symlink and special targets
- Defined explicit reference-only import behavior that makes no byte/hash completeness claim
- Closed duplicate-media plan selection ambiguity through validated evidence/declaration mapping
- Added format, plan, CLI, failure/recovery, unsafe-path, stale, and UG-style fixture regressions
- Passed full tests, focused media tests, fmt, clippy, and Claude judgment

### Files Changed

- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- crates/brain-brew-formats/tests/crowdanki_import_plan.rs
- crates/brain-brew-cli/src/commands/import.rs
- crates/brain-brew-cli/src/help.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-cli/tests/crowdanki_import_plan_cli.rs
- crates/brain-brew-cli/tests/crowdanki_import_media_cli.rs
- crates/brain-brew-cli/tests/ug_style_fixture.rs
- documentation/docs/authoring/importing-crowdanki.md
- documentation/docs/concepts/media.md
