# ADR-004: Identify Deck Entities with Human-Readable Stable IDs

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Deck entities need stable identity across source files, overlays, imports, exports, and releases. Names change, rows move, content changes, and external formats use their own identifiers. For example, Anki and CrowdAnki IDs are important for compatibility, but they are adapter-specific and should not become Brain Brew's primary identity model.

Content hashes are useful for detecting change, but they are not identity: editing a card template or correcting a typo must not create a different conceptual entity.

## Decision

Use human-readable Stable IDs as the primary identity for Canonical Deck entities.

Store adapter IDs separately under their adapter namespace. Use content hashes only for change detection, drift checks, or diagnostics. During import, Brain Brew may suggest Stable IDs, but maintainers must be able to review and correct them before they become canonical source.

## Rationale

**Pros:**

- Stable IDs express maintainer intent: this is the same entity across releases.
- Human-readable IDs make source, overlays, diffs, and conflicts easier to review.
- Adapter IDs can be preserved without leaking external identity rules into the core model.
- Language-neutral Stable IDs let translated decks target the same conceptual entities.

**Cons:**

- Maintainers need to choose or review IDs during migration and import.
- Renaming IDs is a semantic operation that tooling must handle carefully.
- Human-readable IDs need validation to avoid collisions and ambiguity.

## Alternatives Considered

- **Content-based identity**: rejected because content edits would break identity and review-history continuity.
- **Adapter IDs as canonical identity**: rejected because they are format-specific and not always present.
- **Generated UUIDs only**: rejected because they are hard to review and poor overlay targets.
- **Display names or row numbers**: rejected because they change for ordinary editorial reasons.

## Implications

- Canonical Deck collections are keyed by Stable ID where practical.
- Overlays, manifests, semantic diffs, and translation targets refer to Stable IDs.
- Import workflows need suggested-ID review rather than silently committing guessed IDs.
- Adapter export must map Stable IDs to adapter IDs without confusing the two.
