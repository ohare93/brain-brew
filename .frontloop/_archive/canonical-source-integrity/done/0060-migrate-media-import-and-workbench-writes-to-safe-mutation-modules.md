---
title: Migrate media import and Workbench writes to safe mutation modules
priority: high
---

## Goal

Move media hash/images-to-refs, CrowdAnki import output, and Workbench Apply onto shared source-document and workspace-transaction semantics.

## Acceptance Criteria

- Media commands perform complete validation before changing any source
- Import refuses overwrite without explicit force and writes transactionally
- Workbench edits cannot bypass canonical document emission
- Locked dependency sources cannot be selected for mutation
- Failure injection covers every command family and leaves no partial output

## Implementation Notes

Depends on source-document/transaction modules plus package ownership work for locked dependencies.


## Blocked

Blocked on package-federation-security/0010–0070: safe path authorization, immutable lock containment, registry-aware provenance planning, and federated media ownership must land before mutators can prove locked dependency sources are unselectable. Resume this task immediately after those prerequisites integrate.


## Completion Summary

- Completed media mutator audit and retained typed source-document, ownership, and recoverable transaction-only writes
- Migrated CrowdAnki import to typed canonical source construction, default overwrite refusal, explicit --force, safe authorization, expected-state backups, and one recoverable transaction
- Migrated every development Workbench source, translation, metadata, include, and new-language write to typed source documents and one journaled transaction
- Removed generic YAML AST, line mutation, temp/partial rename, and direct canonical write paths
- Added root/locked ownership guards and exact proposed-state validation before commit
- Fixed judge-found intra-request T0 fingerprint and absent-target publication races, preserving external bytes with typed conflicts
- Added command-family failure/restart and root/include/overlay/manifest/new-file race regressions
- Passed full default/dev tests, fmt, both clippy modes, UI/embed, 13 E2E, 74+26 fixture verification, docs, release smoke, and Claude re-judgment

### Files Changed

- crates/brain-brew-cli/src/args.rs
- crates/brain-brew-cli/src/help.rs
- crates/brain-brew-cli/src/commands/import.rs
- crates/brain-brew-cli/src/commands/workbench.rs
- crates/brain-brew-cli/src/workspace_mutation.rs
- crates/brain-brew-cli/src/workspace_transaction.rs
- crates/brain-brew-cli/tests/cli.rs
- crates/brain-brew-formats/src/source_document.rs
- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/src/overlay_source_document.rs
- crates/brain-brew-formats/tests/source_documents.rs
- crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs
- documentation/docs/reference/cli.md
- documentation/docs/reference/workbench.md
- documentation/docs/reference/workspace-transactions.md
