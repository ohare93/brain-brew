# ADR-024: Store Media as External Assets with References

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

CanonicalDeck files are single human-editable YAML files, but real decks include images, audio, and other binary media. Embedding media bytes in YAML would damage readability, diffs, and formatter behavior.

## Decision

Media assets remain external files. CanonicalDeck stores media references with stable IDs, paths, and content hashes for verification and change detection. Note fields and templates keep raw Anki-compatible text; validation extracts media usages from that text and checks them against the media references.

## Rationale

This keeps the deck source readable while still making media part of the deck's validated structure. Hashes let the system detect missing or changed media without turning the canonical source file into a binary container.

## Implications

- A CanonicalDeck file is the structured source of truth, but a complete deck workspace also includes media assets.
- Export adapters must gather referenced media files.
- Validation must detect missing or hash-mismatched media when media checking is requested.
