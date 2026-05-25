# ADR-013: Use Stable IDs as Primary Identity

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

ADR-007 made content-based identification the primary sync identity to avoid polluting clean source files. That is attractive for note-taking workflows, but deck federation and Anki-compatible round-tripping depend on preserving review history and matching entities across releases even when content changes.

## Decision

Use stable explicit IDs as the primary identity for notes and other deck entities. Content hashes are used for change detection, cache keys, and import heuristics, but they do not define entity identity.

## Rationale

A content hash changes when the content changes, which is exactly when a maintainer most needs the system to know that the entity is still the same note, template, or media reference. Stable IDs match Anki/CrowdAnki's review-history model and make federation overlays robust across corrections, translations, and source reorganizations.

## Implications

- ADR-007 is superseded for deck federation and round-trip workflows.
- Importers may infer IDs during bootstrapping, but persisted deck entities need stable IDs thereafter.
- Source formats must either carry stable IDs directly or have a reliable sidecar/mapping strategy.
