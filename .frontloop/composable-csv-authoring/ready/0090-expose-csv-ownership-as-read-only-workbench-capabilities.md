---
title: Expose CSV ownership as read-only Workbench capabilities
priority: high
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-9
---

## Goal

Let Workbench browse, compare, report, and preview CSV-backed notes and translations normally while clearly preventing unsupported writes and retaining editability for units already moved to YAML.

## Acceptance Criteria

- Workbench note, source-string, card, metadata, comparison, and coverage read paths consume materialized CSV-backed data without special output semantics
- Return source capability/provenance for relevant note fields and translation entries, including descriptor, CSV file, row, and column where available
- Disable or reject source/translation edits for CSV-owned units with a typed actionable error instead of attempting CanonicalSourceDocument YAML mutation
- Keep inline YAML-owned units in the same target writable when normal write-mode and transaction requirements are satisfied
- Include all CSV dependencies in workspace freshness and compare-and-swap input fingerprints
- Ensure Apply preview cannot stage or confirm a write that crosses into CSV ownership
- Add RGR API/UI tests proving read-only CSV rows, writable migrated YAML rows, previews, stale workspace invalidation, and fail-closed mixed apply behavior

## Design Decisions

- Read-only is per ownership unit, not necessarily per entire workspace
- CSV write-back, byte-preserving rewrites, and new-language CSV column creation remain unsupported
- Capability enforcement belongs server-side; UI disabling is not the security boundary

## Implementation Notes

Depends on provenance plus mixed note/translation ownership. Coordinate with workbench-hardening tasks, especially typed contracts, fingerprints, immutable preview, and canonical transactions. Do not broaden this task into general Workbench refactoring.
