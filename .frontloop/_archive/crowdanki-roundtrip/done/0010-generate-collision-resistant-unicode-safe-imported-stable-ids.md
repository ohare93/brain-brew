---
title: Generate collision-resistant Unicode-safe imported stable IDs
priority: critical
---

## Goal

Replace ASCII-only first-field slugging with deterministic Unicode-aware suggestions and explicit collision handling.

## Acceptance Criteria

- Non-Latin notes do not collapse to `note.unnamed`
- Suggestions are deterministic across platforms and locale settings
- Collisions receive stable disambiguation or require explicit override
- Diagnostics never advertise a nonexistent override workflow
- Tests cover Cyrillic, CJK, RTL, repeated first fields, blanks, and normalization equivalents

## Implementation Notes

First CrowdAnki task; preserve source GUIDs independently of suggested stable IDs.


## Completion Summary

- Replaced ASCII-only imported-note slugging with NFC-normalized, locale-independent suggestion generation
- Prevented non-Latin and blank first fields from collapsing to note.unnamed
- Added deterministic GUID-assisted digest disambiguation for repeated readable slugs and fallback names
- Kept CrowdAnki GUID as an independent adapter ID rather than conflating it with the suggested stable ID
- Made input-order-independent collision grouping and fail-closed final-ID collision handling explicit
- Removed nonexistent override wording and documented the automatic-only import policy and re-import stability tradeoff
- Added unit, importer, and CLI tests covering Cyrillic, CJK, RTL, blanks, repeats, normalization equivalence, GUID preservation, duplicate GUID rejection, and ordering
- Passed default full workspace tests, targeted format/CLI tests, fmt, clippy, docs, release smoke, parallelism check, and Claude judgment

### Files Changed

- Cargo.toml
- Cargo.lock
- crates/brain-brew-formats/Cargo.toml
- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- crates/brain-brew-cli/tests/cli.rs
- documentation/docs/authoring/importing-crowdanki.md
- documentation/docs/concepts/media.md
- documentation/docs/reference/cli.md
- documentation/sidebars.js
