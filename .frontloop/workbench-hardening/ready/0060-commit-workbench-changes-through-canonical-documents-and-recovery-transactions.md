---
title: Commit Workbench changes through canonical documents and recovery transactions
priority: critical
---

## Goal

Make every Apply include-preserving, canonical, all-or-recoverable, and constrained to authorized files.

## Acceptance Criteria

- No Workbench route serializes generic serde YAML for source edits
- All edits use canonical source-document operations
- All files commit through the journaled workspace transaction module
- Injected rename/crash failures roll back or recover on restart
- The prior include-bearing deck and partial-rename reproductions pass as regressions

## Implementation Notes

Depends on canonical-source-integrity modules, CAS, and preview binding.
