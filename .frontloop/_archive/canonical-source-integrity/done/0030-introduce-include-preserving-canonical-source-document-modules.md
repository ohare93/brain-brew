---
title: Introduce include-preserving canonical source document modules
priority: critical
---

## Goal

Create deep `CanonicalSourceDocument` and `OverlaySourceDocument` modules in formats that expose validated edits while preserving include directives and canonical emission.

## Acceptance Criteria

- Deck and overlay documents preserve scalar/media includes through unrelated edits
- Typed operations cover field, translation, media, and metadata changes needed by current mutators
- All operations enforce duplicate, union, and canonical invariants
- The interface returns source/schema-aware errors without filesystem dependencies
- Round-trip and edit-locality tests demonstrate unchanged unrelated bytes or defined canonical changes

## Implementation Notes

Depends on strict decoders; this becomes the sole source-mutation seam for later tasks.


## Completion Summary

- Added pure include-preserving CanonicalSourceDocument and OverlaySourceDocument modules in brain-brew-formats
- Kept YAML representation private while exposing typed metadata, field, translation, media, image, stale-resolution, and import construction edits
- Routed included scalar/media edits to provenance-tagged outputs while preserving unrelated directives and canonical root bytes
- Made edits atomic by validating cloned state before commit and reused strict duplicate/union/scalar/canonical invariants
- Added exact-byte locality, include routing, source/schema diagnostics, translation/media/image, and idempotence tests
- Passed focused formats tests, full repository tests, fmt, clippy, and independent Claude judgment

### Files Changed

- crates/brain-brew-formats/src/canonical_source_document.rs
- crates/brain-brew-formats/src/overlay_source_document.rs
- crates/brain-brew-formats/src/source_document.rs
- crates/brain-brew-formats/src/source_includes.rs
- crates/brain-brew-formats/src/lib.rs
- crates/brain-brew-formats/tests/source_documents.rs
