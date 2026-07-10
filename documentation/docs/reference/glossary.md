---
title: Glossary
---

# Glossary

## Brain Brew

A local-first deck federation and round-trip system for flashcard decks.

## Deck

A shareable flashcard collection including notes, note types, card templates, styling, metadata, and media references.

## Federated Deck

A composable source package for a shared deck contribution, containing a base deck, overlays, or both.

## Canonical Deck

The format-independent representation of a deck's notes, note types, card templates, styling, metadata, and media references.

## Stable ID

A human-readable identifier that says a deck entity is the same entity across source files, overlays, exports, and releases.

## Adapter ID

An identifier used by an external deck format or tool for the same deck entity, such as a CrowdAnki GUID.

## Overlay

A bounded set of changes applied to a base deck without replacing the base deck.

## Translation Overlay

An overlay that changes deck language or localized text.

## Extension Overlay

An overlay that adds new deck content or structure.

## Patch Overlay

An overlay that corrects or adjusts existing deck content or structure.

## Personal Overlay

An overlay containing learner-specific deck content or structure that should survive shared deck updates.

## Field Fill

An overlay shorthand for filling existing blank note fields with new content while requiring the upstream field to still be blank.

## Source Variable

A named text value referenced from source text with `${variable.name}` before adapter export.

## Translation Dictionary

A translation overlay section mapping exact source text, source variables, and adapter IDs to translated values.

## Expected Base

The prior deck value or condition an overlay declares before making a destructive or conflict-resolving change.

## Build Target

A named composition goal that resolves a base deck and selected overlays into a Resolved Deck.

## Resolved Deck

The deck produced by applying an overlay stack to a base deck.

## Semantic Diff

A comparison of decks by stable IDs and deck entities rather than raw source lines.

## Media Reference

A deck entity that identifies and verifies an external media asset.

## Tombstone

A typed record that an exact deck entity/value address was deliberately removed. Its full path preserves parent scope, and composition-created records retain the removing overlay provenance.
