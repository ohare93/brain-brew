---
title: Allow sparse overlay field values to come from CSV
priority: high
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-10
---

## Goal

Cover data such as UG's Experimental region-code column without duplicating it into generated YAML by allowing a narrowly scoped CSV source to materialize sparse field-addition values in an extension overlay.

## Acceptance Criteria

- Add a source-preserving CSV declaration at the existing field-addition values boundary rather than a generic legacy recipe mechanism
- Map explicit stable note IDs to one or more explicitly declared added fields using the same descriptor, joins, parameters, authorization, and provenance rules
- Validate note existence, field-definition ownership, completeness policy, value types, duplicate ownership, and expected-base/composition invariants through existing overlay semantics
- Preserve direct inline field-addition values and permit disjoint gradual ownership transfer with fatal collisions
- Add a fixture for an Experimental-style region-code field and prove equal composition before and after moving selected values to YAML
- Ensure formatting preserves the declaration and Workbench capabilities report CSV ownership correctly
- Use red-green-refactor and retain regression tests for sparse rows, unknown notes, collisions, and output equality

## Design Decisions

- This is a narrow reuse of CSV source materialization at a real Canonical overlay seam
- Do not implement recursive derivatives, arbitrary transforms, note-model selection, CSV generation, or the old recipe DSL

## Implementation Notes

Depends on the shared descriptor/materializer, CLI provenance, and mixed ownership rules. Keep this after base notes and translations so the abstraction is proven by concrete consumers rather than generalized speculatively.
