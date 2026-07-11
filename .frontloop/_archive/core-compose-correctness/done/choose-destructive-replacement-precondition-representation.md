---
title: Choose destructive replacement precondition representation
priority: critical
---

## Goal

Choose the semantic precondition complete entity replacements must compare against instead of merely checking `Option::is_some()`.

## Acceptance Criteria

- Select canonical entity summaries, stable fingerprints, sparse property-only replacement, or removal of complete replacement interfaces
- Define compatibility and migration behavior for existing overlay YAML
- Specify mismatch diagnostics and override interaction
- Record examples for note types, notes, fields, templates, and media

## Implementation Notes

Blocks expected-base implementation.

## Questions

### Q1: Recommended: use stable canonical fingerprints for complete entities while retaining typed property-level expected values for sparse changes. Which representation should become authoritative?
