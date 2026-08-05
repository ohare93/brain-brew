---
title: Support gradual CSV-to-YAML translation ownership transfer
priority: critical
frontloop_approval_task: b46bc3fb2c1d6c7f087152d6dfedb54566a9a6412c48c0483aa5f92089db1097-8
---

## Goal

Permit individual source translations or whole note contexts to leave CSV ownership and become native YAML entries, gaining native editing and stale detection without changing composed targets.

## Acceptance Criteria

- Support explicit exclusion of imported translation source texts and stable note/path contexts as defined by the ADR
- Prevent an imported global direct/no-change entry derived from remaining rows from leaking back into an excluded occurrence; materialize included occurrences contextually when required to preserve the ownership boundary
- Fail on unknown exclusions, conflicting imported/inline ownership, and incomplete transfers that would change strict translation coverage unexpectedly
- Retain ordinary direct deduplication whenever every occurrence remains CSV-owned
- Add a fixture that moves one reusable translation and one complete country context from CSV to YAML and proves equal composed deck, coverage classification where applicable, and CrowdAnki output
- Prove that the moved native YAML entry subsequently participates in normal stale-source detection while CSV-owned pairs retain the documented limitation
- Expose stable import provenance in reports so users can see which entries remain CSV-owned
- Use red-green-refactor for ownership leakage, equality, and stale-behavior tests

## Design Decisions

- Partial migration is an explicit supported workflow, not an accidental override
- A storage-only ownership move must preserve output
- Native YAML owns excluded units completely; imported global entries must not shadow that ownership

## Implementation Notes

Depends on CSV translation materialization and mixed note provenance. Prefer the smallest exclusion model that supports source-text and note/path transfer; do not add arbitrary filtering expressions.
