# ADR-006: Compose Federated Decks with Ordered Sparse Overlays

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Shared decks often need derivative forms: translations, extensions, corrections, local variants, and learner-specific additions. Copying the whole deck for each variant makes upgrades painful and hides which changes belong to the derivative deck. Ultimate Geography-style language and extension targets need a way to compose changes while preserving a clear base.

## Decision

Represent derivative deck changes as overlays applied to a base Canonical Deck.

An overlay is a sparse Canonical Deck-shaped fragment that targets deck entities by Stable ID and declares only the properties it changes. Brain Brew composes an ordered overlay stack into a resolved deck. Translation overlays, extension overlays, patch overlays, and personal overlays are overlay categories, not separate deck formats.

## Rationale

**Pros:**

- Keeps base decks and derivative contributions separate.
- Makes variants composable without duplicating the whole deck.
- Lets overlays target notes, note types, card templates, metadata, media references, and other deck entities consistently.
- Gives maintainers a reviewable diff of a contribution's intent.

**Cons:**

- Overlay authors must understand the base deck's Stable IDs.
- Stack order matters and needs to be visible.
- Composition errors can be more complex than editing a copied deck.

## Alternatives Considered

- **Full deck copies for each variant**: rejected because they drift and make upstream updates hard.
- **Format-specific patches**: rejected because they bind federation to CrowdAnki or another adapter format.
- **Imperative transformation scripts**: rejected because they are harder to validate, diff, and invert.
- **Unordered automatic merging**: rejected because it hides conflicts and makes results unpredictable.

## Implications

- Composition is a core domain behavior in `brain-brew-core`.
- Overlay YAML is a sparse representation decoded by `brain-brew-formats`.
- Build targets select a base deck and an ordered overlay stack.
- Semantic diffs and diagnostics should explain overlay effects in deck-entity language.
