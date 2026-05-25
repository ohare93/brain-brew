# ADR-032: Represent Removals as Tombstones

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

In a federated shared deck, absence is ambiguous: an entity may be missing because it was never present, because an import was incomplete, or because an overlay deliberately removed it. Silent physical deletion weakens diffs and upgrade reasoning.

## Decision

Overlay remove intent creates a tombstone in the resolved deck. CrowdAnki export initially omits tombstoned entities and reports them as intentional removals. The resolved CanonicalDeck retains the fact that the removal was intentional.

## Rationale

Tombstones make removals auditable and distinguish deliberate deletion from accidental loss. They also give future upgrade and personal-overlay workflows a stable way to reason about removed deck entities.

## Implications

- CanonicalDeck validation must understand tombstoned entities.
- Semantic diffs should report tombstones distinctly from missing entities.
- Export adapters decide how tombstones map to target formats.
