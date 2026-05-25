# ADR-036: Model Deck Variants as Extension Overlays

**Date**: 2026-05-23  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Shared decks often publish variants: standard vs extended cards, beginner vs advanced views, regional editions, or other maintained configurations. These variants may change card-generation structure and adapter identities while still representing the same conceptual notes and note types.

The Ultimate Geography parity fixture is the motivating case: its Extended variant adds card templates and uses different CrowdAnki deck and note-model UUIDs in the legacy output. Brain Brew must reproduce that output, but Ultimate Geography is a test case for the general federation model rather than a product-specific application feature.

## Related Decisions

- [ADR-014: Allow Overlays to Target Any Deck Entity](0014-allow-overlays-to-target-any-deck-entity.md) - variants may need to change note types, templates, metadata, and adapter IDs.
- [ADR-016: Represent Overlays as Sparse Canonical Deck Fragments](0016-represent-overlays-as-sparse-canonical-deck-fragments.md) - variants should describe only their changes from the base deck.
- [ADR-035: Use Language-Neutral Stable IDs for Translated Targets](0035-use-language-neutral-stable-ids-for-translated-targets.md) - canonical identity should remain stable across translated and variant targets.

## Decision

Model deck variants as Extension Overlays on the same canonical deck entities when the variant represents the same conceptual deck with added or changed structure.

For the Ultimate Geography parity fixture, Extended is an Extension Overlay on the same canonical deck and note type as Standard. The overlay adds additional card templates and changes target-specific adapter identities needed for CrowdAnki export parity. It does not create a separate canonical note type solely because the legacy exported CrowdAnki model UUID differs.

## Rationale

A variant overlay avoids duplicating notes, fields, common templates, styling, and media references. It keeps future Federated Decks able to target the same stable note and note-type identities regardless of selected variant.

External UUID differences remain adapter IDs and can vary by target without forcing separate stable IDs.

## Alternatives Considered

- **Separate note type entity for each variant**: closer to some legacy exported identities, but duplicates structure and weakens overlay semantics.
- **Separate base deck for each variant**: simplest for adapter parity, but contradicts the desired reproducible base-plus-overrides workflow.

## Implications

- Overlay support must allow card-template additions and adapter-ID changes on existing note types.
- The Ultimate Geography fixture should compose Extended targets by applying an Extension Overlay to the English Standard base.
- CrowdAnki parity tests should confirm that adapter IDs still produce legacy variant deck/model identities on export.
