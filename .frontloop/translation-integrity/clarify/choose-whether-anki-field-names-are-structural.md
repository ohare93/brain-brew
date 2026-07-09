---
title: Choose whether Anki field names are structural
priority: critical
---

## Goal

Decide whether field-definition names are non-translatable identifiers or may translate only with atomic Mustache/reference rewrite and validation.

## Acceptance Criteria

- Choose structural or fully rewritable semantics
- Define migration for existing field-name dictionary entries
- Specify validation for templates and other Anki references
- Document UI/coverage behavior for field labels versus identifiers

## Implementation Notes

Blocks field-name safety implementation.

## Questions

### Q1: Recommended: make Anki field names structural and non-translatable; introduce separate display-label content if needed. Approve or require atomic reference rewriting?
