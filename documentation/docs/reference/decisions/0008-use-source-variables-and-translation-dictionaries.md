# ADR-008: Use Source Variables and Translation Dictionaries

**Date**: 2026-05-25  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Translation overlays become hard to review when a small phrase change requires replacing entire card template HTML blocks. Note field translations are also noisy if every entry repeats replace intent and expected-base boilerplate. At the same time, translations still need drift protection: stale translations should fail when upstream source text changes.

## Decision

Canonical Deck entities may define source variables, and translation overlays may use translation dictionaries.

Text can reference variables with `${variable.name}`. Variables are resolved from the most specific scope to the broadest scope: card template, note, note type, then deck. Translation dictionaries map faithful direct reusable source strings, faithful contextual source-string translations scoped by stable deck path, explicit no-change decisions for reviewed text that intentionally stays identical, variable values, and adapter IDs to translated values. Top-level `target_adaptations` hold typed path-scoped target-language adaptations or deletions. They declare `intent`, translation `ownership`, an explicit `expected_source`, and a review `reason`; adaptations carry non-blank target text and deletions deliberately omit target text. The `direct`/`contextual` source keys and `no_change` source entries act as expected bases.

CrowdAnki and other adapter exports render variables before writing output, so distributable decks contain ordinary adapter-compatible text and HTML.

## Rationale

**Pros:**

- Keeps shared card template structure in the base deck.
- Lets language overlays translate phrase values without copying whole templates.
- Makes translations readable as source text next to translated text.
- Fails stale entries when the source text or target path no longer matches.
- Supports context-specific translations where the same source phrase needs different target text.
- Distinguishes intentionally unchanged translator-reviewed text from genuinely missing translations.

**Cons:**

- Variable scoping adds one more concept to Canonical Deck source.
- Translation extraction and coverage checks must be deterministic.
- Ambiguous source strings need `contextual` dictionary entries; no-change remains reusable exact-source coverage.

## Alternatives Considered

- **Replace full localized templates**: rejected because template maintenance would fork per language.
- **Per-field replace objects everywhere**: rejected because translation overlays become too verbose.
- **Runtime Anki variables**: rejected because exports should remain plain Anki-compatible decks.
- **External translation spreadsheets as the source of truth**: rejected because Brain Brew needs source-controlled overlay semantics.

## Implications

- Translation overlays should prefer variable and dictionary changes over structural template forks.
- Stale direct/path-specific dictionary entries, stale no-change entries, and target adaptations whose `expected_source` no longer matches fail composition.
- Export parity is based on rendered deck semantics, not on preserving variable syntax in adapter output.
- Documentation and skills should teach the variable-first workflow for UG-style variants.
