---
title: Unify translation reporting resolution and JSON behavior
priority: medium
---

## Goal

Make CLI and Workbench expose the same precedence, stale resolution, hidden/structural classification, coverage totals, and machine-readable failures.

## Acceptance Criteria

- One resolver/report model powers CLI and Workbench
- `translations --json` always returns valid structured success/error output
- Structural and hidden units are categorized rather than inflating fallback counts ambiguously
- Path/context lookup remains deterministic and has scale tests
- Documentation examples are generated from the report schema

## Implementation Notes

Final translation platform task after policy implementation.


## Blocked

Agentleman worker launch is infrastructure-blocked: `launchPresets.worker-write-jack uses retired {agentlemanArgs} launch composition; v2 argv must be a complete opaque command.` Both direct `agm run` and visible delegation fail before creating a run. Translation 0050 implementation has not started; repair the Agentleman worker-write launch preset, then resume this final translation task.


## Completion Summary

- Unified final-stack coverage resolver for CLI verification and Workbench views
- Added deterministic mutually exclusive final-stack status totals, including hidden and structural exclusions
- Added versioned JSON success envelopes and structured JSON-only error output for translations commands
- Added large-path deterministic report tests and CLI schema/error tests
- Schema-locked documentation examples to the supported JSON envelope
- Passed full CI and independent Claude judgment

### Files Changed

- crates/brain-brew-core/src/model.rs
- crates/brain-brew-core/src/translation.rs
- crates/brain-brew-core/tests/overlay_compose.rs
- crates/brain-brew-cli/src/commands/translations.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/tests/translations_cli.rs
- documentation/docs/authoring/translations.md
