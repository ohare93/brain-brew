---
title: Migrate formatting and translation mutations to source documents
priority: high
---

## Goal

Remove direct serde-value and line-surgery paths from `fmt` and translation insertion/resolution, using the canonical source-document and transaction modules.

## Acceptance Criteria

- Formatting and translation writes use typed document operations
- Include-bearing files remain canonical after edits
- All multi-file translation operations are plan-first and recoverable
- Existing formatting/idempotence and stale-translation behavior remain green
- Direct mutation implementations are deleted rather than retained as fallbacks

## Implementation Notes

Depends on source-document and transaction modules.


## Completion Summary

- Migrated canonical and overlay fmt paths to typed source documents with strict validation and canonical emission
- Migrated all CLI translation apply, interactive/contextual insertion, stale confirm/replace/batch, and source-impact writes to typed overlay operations
- Added plan-first affected-target composition validation, expected fingerprints, recovery-first behavior, and one journaled transaction per operation
- Removed legacy YAML line-surgery, serde-value mutation, and direct command write fallback implementations
- Preserved unrelated scalar includes, canonical idempotence, stale precedence, and shadowed current translation decisions
- Documented canonical rewrite and transaction behavior; retained format-specific strict codecs where appropriate
- Passed full fmt/test/clippy, focused CLI/formats/UG suites, and independent Claude judgment

### Files Changed

- crates/brain-brew-cli/src/commands/fmt.rs
- crates/brain-brew-cli/src/commands/translations.rs
- crates/brain-brew-cli/src/io.rs
- crates/brain-brew-cli/src/main.rs
- crates/brain-brew-cli/src/workspace_mutation.rs
- crates/brain-brew-cli/src/workspace_transaction.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-cli/tests/translations_cli.rs
- crates/brain-brew-formats/src/overlay_source_document.rs
- crates/brain-brew-formats/tests/source_documents.rs
- documentation/docs/authoring/translations.md
- documentation/docs/reference/workspace-transactions.md
