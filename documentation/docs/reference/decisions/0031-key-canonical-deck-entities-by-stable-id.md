# ADR-031: Key Canonical Deck Entities by Stable ID

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

CanonicalDeck files need to make stable identity obvious and support deterministic formatting, overlay targeting, and semantic diffs. Some deck entities, especially note fields and card templates, also have meaningful order for Anki-compatible exports.

## Decision

CanonicalDeck YAML organizes deck entities as maps keyed by stable ID. Where order is semantically meaningful, such as fields and card templates within a note type, the model uses explicit order arrays rather than relying on map order.

## Rationale

Stable IDs are the primary identity, so making them YAML keys keeps the source format aligned with the domain model. Explicit order arrays preserve Anki-compatible ordering without making identity depend on list position.

## Implications

- Canonical formatting can sort maps deterministically by stable ID.
- Validation must ensure order arrays reference existing IDs exactly once where required.
- Overlays target entities naturally by path and stable ID.
