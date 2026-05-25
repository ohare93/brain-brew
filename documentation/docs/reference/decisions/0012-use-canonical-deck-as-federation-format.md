# ADR-012: Use Canonical Deck as Federation Format

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Earlier decisions centered the architecture on `CanonicalNote`. Reassessing Brain Brew and Ultimate Geography showed that the hard problem is not just note content: shared decks also include note types, card templates, styling, deck metadata, media references, and release structure. Extensions and translations may need to affect any of those parts.

## Decision

Use **CanonicalDeck** as the central federation format. A CanonicalDeck represents notes, note types, card templates, styling, metadata, and media references as one coherent deck package. Individual notes remain important entities inside that package, but they are not the top-level federation unit.

## Rationale

A note-only hub would reproduce Brain Brew's limitations in a new form: it would make note content composable while leaving note models, templates, and media trapped in adapters. CanonicalDeck keeps the hub-and-spokes idea but moves the hub to the correct domain boundary.

## Implications

- ADR-002 and ADR-009 are superseded where they identify `CanonicalNote` as the central federation format.
- Adapters convert to and from CanonicalDeck, not just lists of notes.
- The first milestone should prove deck-level round-trip fidelity, including note types/templates/media references, not only note fields.
