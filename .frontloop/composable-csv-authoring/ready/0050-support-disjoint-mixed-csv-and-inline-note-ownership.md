---
title: Support disjoint mixed CSV and inline note ownership
priority: critical
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-5
---

## Goal

Enable the gradual migration promised by the design: selected notes can leave CSV ownership and become direct YAML while the rest continue to materialize from the same CSV source with identical resolved output.

## Acceptance Criteria

- Implement the ADR's composable notes source expression for direct maps, CSV sources, and mixed disjoint ownership
- Allow CSV declarations to explicitly exclude stable note IDs transferred to inline YAML
- Fail on duplicate ownership, unknown exclusions, missing transferred notes, and any implicit override attempt
- Preserve deterministic source emission without expanding the remaining imported notes
- Add a fixture that begins fully CSV-backed, moves one complete country/note to inline YAML, and proves equal CanonicalDeck semantic diff and equal adapter output
- Track per-note and per-field authoring provenance needed by later capability handling
- Use red-green-refactor, including an initial failing equivalence test and collision regressions

## Design Decisions

- Ownership transfer requires explicit exclusion from CSV plus explicit inline definition
- Source order never decides ownership
- The resolved deck before and after a pure storage migration must be semantically equal

## Implementation Notes

Depends on note materialization and preferably explicit joins. Keep source composition specific to note maps rather than introducing a general-purpose YAML merge language.
