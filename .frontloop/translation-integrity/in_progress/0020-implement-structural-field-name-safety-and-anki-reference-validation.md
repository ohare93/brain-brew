---
title: Implement structural field-name safety and Anki reference validation
priority: critical
---

## Goal

Apply the chosen field-name policy and ensure no localized export can retain stale `{{Field}}` references.

## Acceptance Criteria

- Field names are excluded from translation or all references are atomically rewritten according to decision
- Template validation resolves Mustache field references against the final note-type schema
- Coverage no longer encourages unsafe identifier translation
- Existing dictionaries receive migration diagnostics
- The Capital→Hauptstadt reproduction fails safely or exports fully rewritten cards

## Implementation Notes

Depends on field-name decision and canonical source migration support.
