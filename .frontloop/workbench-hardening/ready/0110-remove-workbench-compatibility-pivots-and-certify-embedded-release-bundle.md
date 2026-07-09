---
title: Remove Workbench compatibility pivots and certify embedded release bundle
priority: medium
---

## Goal

Delete broad legacy routes after all clients use typed list/detail interfaces and prove the embedded shipped UI—not only dev assets—passes the complete browser suite.

## Acceptance Criteria

- Card/source/metadata/comparison compatibility pivots have no clients and are removed
- Route removal is reflected in the versioned contract and docs
- All 13+ hardened browser scenarios run against embedded release assets
- Asset freshness remains byte-checked
- Apply experimental/read-only containment is removed only when all safety exit criteria pass

## Implementation Notes

Final Workbench task.
