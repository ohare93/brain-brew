---
title: Centralize translation dictionary mutation invariants in core
priority: critical
---

## Goal

Replace public-map surgery duplicated in CLI and Workbench with domain commands for direct/contextual/no-change/stale/adaptation changes.

## Acceptance Criteria

- Core exposes typed commands such as set_direct, set_contextual, set_no_change, record_source_change, and resolve_stale
- Commands enforce cross-map exclusivity and precedence
- CLI and Workbench use the same policy implementation
- Property tests cover command sequences and canonical results
- Public raw-map mutation is reduced or clearly internalized

## Implementation Notes

Can start while policy decisions resolve, but final command semantics must implement them.
