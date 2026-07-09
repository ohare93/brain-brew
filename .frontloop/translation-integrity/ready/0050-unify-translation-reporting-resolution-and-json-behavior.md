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
