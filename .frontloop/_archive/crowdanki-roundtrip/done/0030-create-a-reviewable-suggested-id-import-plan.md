---
title: Create a reviewable suggested-ID import plan
priority: high
---

## Goal

Replace blanket `--accept-suggested-ids` with an inspectable plan/override artifact supporting selective approval and repeatable imports.

## Acceptance Criteria

- A dry-run emits proposed note/note-type/template IDs with source GUID/model evidence
- Maintainers can override selected suggestions in a documented file
- Import refuses unresolved collisions and unreviewed changes according to policy
- Re-running with the same plan is deterministic
- CLI help/docs cover generate, review, apply, and recovery steps

## Implementation Notes

Depends on Unicode-safe suggestion and identity validation.


## Completion Summary

- Replaced blanket --accept-suggested-ids with versioned plan, review, and apply CLI workflow
- Added canonical provenance-bound review-plan schema with source byte fingerprint, import options, source GUID/model/template evidence, proposed IDs, and explicit decisions
- Made plan generation side-effect-free for source decks and apply fail closed on stale source, edited plan evidence, unapproved automatic suggestions, unresolved decisions, and invalid overrides
- Added selective stable-ID overrides with strict syntax, evidence, and global uniqueness validation
- Used existing safe output/workspace transaction and recovery behavior for plan generation and import application
- Removed legacy flag behavior and added actionable migration guidance
- Added deterministic canonical serialization, input-order, stale-plan, collision, override, failure/recovery, format, CLI, and UG-style regressions
- Passed full default tests, focused plan tests, fmt, clippy, docs, release smoke, and Claude judgment

### Files Changed

- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- crates/brain-brew-formats/tests/crowdanki_import_plan.rs
- crates/brain-brew-formats/tests/ultimate_geography_fixture.rs
- crates/brain-brew-cli/src/commands/import.rs
- crates/brain-brew-cli/src/help.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-cli/tests/crowdanki_import_plan_cli.rs
- crates/brain-brew-cli/tests/ug_style_fixture.rs
- documentation/docs/authoring/importing-crowdanki.md
- documentation/docs/concepts/media.md
- documentation/docs/reference/cli.md
