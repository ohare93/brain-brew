# ADR-035: Use Language-Neutral Stable IDs for Translated Targets

**Date**: 2026-05-23  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

The first Ultimate Geography importer materialized each language and variant as a complete CanonicalDeck with target-specific stable IDs such as `deck.ultimate-geography.de.extended` and `note-type.ultimate-geography.de.extended`.

That was useful to prove semantic CrowdAnki parity quickly, but the federation model now treats English Standard as the base and other languages/variants as overlays. In that model, translated targets represent the same conceptual deck entities with different localized content and adapter identities.

## Related Decisions

- [ADR-013: Use Stable IDs as Primary Identity](0013-use-stable-ids-as-primary-identity.md) - stable IDs define sameness across sources and releases.
- [ADR-014: Allow Overlays to Target Any Deck Entity](0014-allow-overlays-to-target-any-deck-entity.md) - language overlays must be able to modify deck metadata, notes, note types, templates, and adapter identities.
- [ADR-029: Use Human-Readable Stable IDs with Separate Adapter IDs](0029-use-human-readable-stable-ids-with-separate-adapter-ids.md) - external CrowdAnki UUIDs/GUIDs remain adapter IDs, not stable IDs.

## Decision

Ultimate Geography translated and variant targets share language-neutral stable IDs for the same conceptual deck entities.

For example, the base deck and translated targets should use stable IDs shaped like:

- `deck.ultimate-geography`
- `note-type.ultimate-geography`
- `note.england`
- `template.country-capital`

Language and variant overlays change localized field values, card template text, deck/note-type names, descriptions, and CrowdAnki adapter IDs as needed for export parity.

## Rationale

Language-neutral stable IDs make translations true overlays instead of separate deck copies. They let future Federated Decks target the same conceptual notes, templates, and media references regardless of language target. They also keep adapter-specific identity separate: old CrowdAnki deck UUIDs, note-model UUIDs, and note GUIDs can still vary per target without redefining canonical sameness.

## Alternatives Considered

- **Target-specific stable IDs**: easiest to preserve the first importer shape, but makes languages behave like independent decks and weakens federation semantics.
- **Hybrid IDs**: share note IDs while keeping target-specific deck or note-type IDs. This reduces some duplication but creates a mixed identity model that is harder to explain and validate.

## Implications

- The Ultimate Geography source materializer should produce language-neutral base and overlay stable IDs.
- Parity tests should compare exported CrowdAnki semantics and composed deck behavior, not rely on target-specific stable ID equality from the first importer.
- Overlay support must include metadata and adapter-ID changes so target-specific CrowdAnki identities can be expressed without changing stable IDs.
