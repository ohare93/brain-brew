# ADR-003: Use Canonical Deck as Federation Format

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

A note-only hub is not enough for shared Anki-compatible decks. Maintainers must preserve and compose note types, card templates, styling, metadata, media references, tags, adapter identities, and notes as one coherent deck package. Translations and extensions often need to change template text, field names, deck metadata, and media references, not only note field values.

## Decision

Use `CanonicalDeck` as Brain Brew's central federation and round-trip format.

A Canonical Deck represents the deck entities Brain Brew understands: notes, note types, card templates, styling, metadata, tags, media references, source variables, and adapter IDs. External formats import into Canonical Decks and export from Canonical Decks.

## Rationale

**Pros:**

- Keeps federation at the same boundary maintainers think about: a deck, not isolated notes.
- Avoids trapping templates, note models, and media inside format-specific adapters.
- Lets overlays target any meaningful deck entity with stable identity.
- Provides one hub for CrowdAnki, canonical YAML, manifests, semantic diffs, and future adapters.

**Cons:**

- The canonical model is larger than a list of notes.
- Import/export fidelity requires more fixtures and tests.
- Unsupported external deck features need explicit handling rather than accidental passthrough.

## Alternatives Considered

- **Canonical Note as the hub**: rejected because it cannot represent whole deck structure.
- **Format-specific federation**: rejected because it couples overlays to CrowdAnki, CSV, or another source format.
- **Adapter passthrough blobs**: rejected because they make composition and validation opaque.

## Implications

- Every format codec converts to or from Canonical Deck, not only note rows.
- Round-trip tests must cover note types, templates, metadata, media references, and notes together.
- Overlay and diff behavior should be expressed in deck-entity terms.
- The glossary and user docs should use “Canonical Deck” as the top-level source concept.
