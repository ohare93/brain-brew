---
title: Normalize CLI output safety and source diagnostics
priority: medium
---

## Goal

Give compose, export, import, diff, help, and JSON modes consistent parent creation, clean-output, overwrite, exit-code, and filename-rich error behavior.

## Acceptance Criteria

- Compose creates declared output parents or reports a precise failure
- Export uses a clean transaction and cannot retain stale files
- Import validates all options and requires explicit overwrite intent
- Parse and schema errors include the relevant source filename
- JSON-capable commands emit structured errors consistently, including translations
- Diff supports an optional change-sensitive exit code and trailing arguments are validated

## Implementation Notes

Final source-integrity UX task after shared transaction behavior exists.


## Completion Summary

- Added recoverable compose/import file publication with safe parent creation and explicit overwrite semantics
- Added clean staged CrowdAnki output-tree publication with full validation, force/backup/journal/recovery, and stale-file elimination
- Made import/help/version/all CLI flag parsing strict for unknown, duplicate, conflicting, and trailing arguments
- Added source/schema/JSON-path-rich diagnostics including CrowdAnki nested paths
- Introduced one versioned structured JSON error envelope across all --json routes including pre-dispatch and translations errors
- Added opt-in diff exit status contract: 0 no changes, 2 differences, 1 operational error
- Added output failure injection and dirty-tree regressions
- Passed full Rust tests, fmt/clippy, E2E, docs, release smoke, CLI/CrowdAnki contract tests, and Claude judgment

### Files Changed

- Cargo.lock
- crates/brain-brew-cli/src/args.rs
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/help.rs
- crates/brain-brew-cli/src/output.rs
- crates/brain-brew-cli/src/output_transaction.rs
- crates/brain-brew-cli/src/workspace_mutation.rs
- crates/brain-brew-cli/src/media_assets.rs
- crates/brain-brew-cli/src/commands/compose.rs
- crates/brain-brew-cli/src/commands/export.rs
- crates/brain-brew-cli/src/commands/import.rs
- crates/brain-brew-cli/src/commands/diff.rs
- crates/brain-brew-cli/tests/cli_contract.rs
- crates/brain-brew-cli/tests/translations_cli.rs
- crates/brain-brew-formats/Cargo.toml
- crates/brain-brew-formats/src/crowdanki.rs
- crates/brain-brew-formats/tests/crowdanki.rs
- documentation/docs/authoring/diff-explain.md
- documentation/docs/authoring/verify-export.md
- documentation/docs/reference/cli.md
- documentation/docs/reference/workspace-transactions.md
