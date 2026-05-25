# ADR-026: Fail on Unsupported Adapter Data

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

CrowdAnki and other adapter formats may contain fields that CanonicalDeck has not modeled. Dropping them would risk data loss, while preserving opaque adapter blobs would weaken the canonical model and make federation semantics unclear.

## Decision

Adapters fail import or export when they encounter unsupported data, and report the unsupported fields/entities precisely. The project should model required data deliberately as real CanonicalDeck concepts rather than hiding it in an opaque escape hatch.

## Rationale

A deck-maintainer tool must be trustworthy. Failing loudly is better than silently losing deck data or carrying uninterpreted blobs that overlays cannot reason about.

## Implications

- Initial CrowdAnki support can be intentionally partial, but must say exactly what is unsupported.
- Fixtures should include only data the canonical model intentionally supports.
- Adapter coverage expands by modeling new concepts, not by adding a generic junk drawer.
