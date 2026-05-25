# ADR-014: Allow Overlays to Target Any Deck Entity

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

If overlays only modify notes, Brain Brew cannot safely express important deck-maintainer changes such as adding a field, changing a card template, updating styling, or adjusting media references. But allowing overlays to touch every part of a deck increases validation and conflict complexity.

## Decision

Overlays may target any entity in a CanonicalDeck by stable ID: notes, note fields, tags, note types, fields, card templates, styling, media references, and deck metadata.

## Rationale

Deck federation is about evolving whole shared decks, not just note text. Ultimate Geography-style extensions and translations may need to change fields, templates, media, and metadata as well as notes. Stable IDs keep these changes addressable without tying them to source file positions or adapter-specific structures.

## Implications

- Overlay validation must understand the kind of entity being changed.
- Conflict detection must work across deck structure, not only note content.
- The first fixture should include at least one non-note overlay target so this remains tested from the start.
