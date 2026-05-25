# ADR-023: Exclude Review State from Canonical Deck

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Anki decks have content and structure, while Anki collections also contain review and scheduling state. Including review history would move Brain Brew back toward being a full sync/backup tool, which conflicts with the narrowed deck federation scope.

## Decision

CanonicalDeck excludes review and scheduling state. Review history is preserved indirectly by maintaining stable note/card identity across exports and imports.

## Rationale

Deck maintainers need to evolve and publish deck content without breaking learners' progress. Stable IDs are the right boundary for that: they let Anki retain history while Brain Brew stays focused on deck content and federation.

## Implications

- Brain Brew is not an Anki collection backup format in the first design.
- Tests should verify stable identity preservation, not review-log round trips.
- Future scheduling-state sidecars can be considered separately if needed.
