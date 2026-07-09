---
title: Unify and translate the duplicated capital hint variable
priority: high
---

## Goal

Remove the two-source-variable mismatch that leaves mixed-language card faces in 13 languages.

## Acceptance Criteria

- Capital hint templates reference one intentional source unit or both units are linked by a documented invariant
- All affected language dictionaries are migrated
- Every 13-language reproduction renders translated hint text consistently
- Extraction/coverage reports contain no accidental duplicate source unit
- Regression tests exercise question and answer templates

## Implementation Notes

Can proceed after source canonicalization; follow variable-first Federated Deck skill.
