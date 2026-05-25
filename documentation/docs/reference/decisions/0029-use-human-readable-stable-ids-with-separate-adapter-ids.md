# ADR-029: Use Human-Readable Stable IDs with Separate Adapter IDs

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Stable IDs are required for overlays and round trips, but Anki/CrowdAnki already have GUIDs that must be preserved for review history. Using raw adapter GUIDs as canonical IDs would make source files harder to maintain and would not generalize cleanly to non-note deck entities.

## Decision

CanonicalDeck uses human-readable stable IDs for deck entities, such as note and template IDs. Adapter-specific identities, such as Anki/CrowdAnki GUIDs, are stored separately as adapter IDs when needed.

## Rationale

Human-readable IDs make overlays and source files maintainable. Separate adapter IDs preserve external-tool identity without letting one adapter's identity scheme dominate the canonical model.

## Implications

- Export adapters must preserve and emit adapter IDs where applicable.
- Import adapters must map external IDs into adapter identity fields and assign or request canonical stable IDs.
- Validation must ensure stable IDs are unique and adapter IDs are not accidentally treated as canonical IDs.
