# ADR-011: Narrow Scope to Local-First Deck Federation

**Date**: 2026-05-22  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

The original ADR set describes several overlapping ambitions: a universal note sync system, a web-first application, content-based note identity, and Dropbox-style bidirectional sync. After reassessing Brain Brew and Ultimate Geography, the clearest valuable slice is deck federation: allowing base flashcard decks to be composed with translations, extensions, patches, and personal overlays without copying the whole deck.

## Decision

For the fresh start, Brain Brew will primarily be a **local-first deck federation and round-trip engine** for flashcard decks. The initial product surface is a CLI/library workflow for deck maintainers, not a SaaS product, web-first GUI, or live sync service.

## Rationale

This focuses the project on the pain proven by Ultimate Geography and Brain Brew: complex source-to-deck builds, derivative CSVs, translations, media, note models, and upgrade-safe customization. It preserves the strongest idea from the old plan—federated decks—while deferring the hardest and least-proven parts, especially live note-system sync.

## Implications

- The canonical model must represent whole decks, not only individual notes.
- Web UI, SaaS, and Dropbox-style live sync are deferred until deck federation works.
- Existing ADRs about web-first deployment and live bidirectional sync should be read as historical context, not near-term implementation scope.
