# ADR-015: Use Ordered Overlay Stack with Explicit Conflicts

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Deck federation needs deterministic composition, but silent last-write-wins would hide data loss when two overlays modify the same deck entity or property. A fully unordered dependency graph is more complex than needed for the first design.

## Decision

Federation uses an ordered overlay stack. Overlays apply in declared order, but conflicting changes to the same deck entity/property fail validation unless the later overlay explicitly declares override intent.

## Rationale

Order gives maintainers a simple mental model and reproducible builds. Explicit conflict failure protects deck content and user changes from accidental overwrites. Explicit override still allows intentional replacement when a maintainer wants one overlay to supersede another.

## Implications

- Recipes must declare overlay order.
- Validation must detect duplicate incompatible changes.
- Override intent is part of the federation language, not an accidental side effect of ordering.
