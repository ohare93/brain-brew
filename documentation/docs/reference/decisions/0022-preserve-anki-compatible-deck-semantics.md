# ADR-022: Preserve Anki-Compatible Deck Semantics

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

CanonicalDeck could simplify the world into generic front/back cards, but the target decks and CrowdAnki workflows distinguish notes, note types, fields, card templates, styling, tags, and generated cards. Ultimate Geography depends on that structure.

## Decision

CanonicalDeck preserves Anki-compatible deck semantics explicitly. Notes, note types, fields, card templates, tags, styling, metadata, and media references are distinct concepts in the core model. Card templates and styling are represented as raw Anki-compatible template/CSS text plus metadata, not as a new Brain Brew template language.

## Rationale

Collapsing to generic cards would make simple decks easy but would lose the structure needed for high-fidelity Anki/CrowdAnki round trips and deck-maintainer federation. The project can later expose simpler facades, but the canonical model must keep the richer semantics.

## Implications

- CrowdAnki fidelity is a core test target.
- Overlay targets include note types and templates, not only note field values.
- Generic card import/export, if added later, will be an adapter concern rather than the canonical model.
