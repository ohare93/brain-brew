# ADR-025: Use Strict Validation by Default

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

A deck federation tool can either accept ambiguous input and try to normalize it, or reject ambiguity before building. Because Brain Brew is meant to protect shared deck structure and learner history, silent normalization can hide destructive mistakes.

## Decision

CanonicalDeck files and overlays are strict by default. Unknown fields, missing stable IDs, invalid references, duplicate IDs, unresolved media, invalid overlay targets, and unresolved federation conflicts fail validation. Explicit extension or escape-hatch mechanisms can be added later.

## Rationale

Strict validation gives deck maintainers predictable builds and protects against accidental data loss. It also keeps the first canonical format honest: anything preserved must be modeled deliberately.

## Implications

- `validate` is a first-class command, not an optional lint.
- Import adapters must either map data into known canonical concepts or report unsupported data clearly.
- Future extension namespaces should be explicit rather than accidental unknown fields.
