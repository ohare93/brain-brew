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
