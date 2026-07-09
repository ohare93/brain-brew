---
title: Implement compositional translation coverage and ownership
priority: critical
---

## Goal

Certify final target stacks across split dictionaries while attributing missing/stale content to the overlay responsible for it.

## Acceptance Criteria

- Strict coverage follows the approved ownership model across the ordered target stack
- Base and extension dictionaries can jointly satisfy completeness without hidden fallback
- Reports identify responsible source overlay, path, source text, and status
- Ignored/no-change/stale/adaptation categories remain explicit
- Main/Hardcore fixture tests cover complete and incomplete stacks

## Implementation Notes

Depends on strict-policy decision and centralized mutation commands.
