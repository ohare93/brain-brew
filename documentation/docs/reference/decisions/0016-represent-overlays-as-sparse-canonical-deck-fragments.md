# ADR-016: Represent Overlays as Sparse Canonical Deck Fragments

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Overlays need to be maintainable by deck authors while still precise enough for validation and conflict detection. Full derived deck snapshots duplicate too much data, while command-only patch lists are exact but can become imperative and hard to read.

## Decision

Represent an overlay as a sparse CanonicalDeck-shaped fragment keyed by stable IDs, with explicit change intent on changed entities/properties: add, merge, replace, remove, or override.

## Rationale

Sparse deck fragments preserve the declarative feel of the project: maintainers describe the deck-shaped result they intend to contribute without copying the whole base deck. Explicit intent keeps conflict handling precise and prevents accidental overwrites.

## Implications

- Overlay files should resemble the CanonicalDeck structure but allow omitted unchanged sections.
- Removal and replacement need explicit markers rather than absence.
- Validation must compare overlay intent against the base deck and earlier overlays in the stack.
