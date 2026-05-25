# ADR-039: Use Source Variables and Translation Dictionaries for Translation Overlays

**Date**: 2026-05-23  
**Status**: Accepted  
**Deciders**: Project Lead

## Context

Translation overlays became hard to review when a small phrase change required replacing entire card template HTML blocks. Note field translations were also verbose because each field repeated `intent: replace`, `value`, and `expected_base.value` even though a translation overlay already means "replace this source text with translated text".

The overlay model still needs drift protection: if upstream English source text changes, stale downstream translations should not silently apply.

## Decision

Canonical Deck entities may define source variables in `variables` maps. Text may reference variables with `${variable.key}`. Variables are scoped from most specific to broadest during rendering:

1. card template variables;
2. note variables;
3. note type variables;
4. deck variables.

Translation overlays may define a `translations` dictionary:

- `changes` maps exact source text from metadata, note fields, note tags, and other extracted deck text to translated text, either globally or scoped to stable deck paths;
- `additions` maps stable deck paths to translated text that may only fill currently blank source values;
- `variables` maps variable keys to exact source-text replacements for context-sensitive phrases;
- `adapter_ids` maps source adapter IDs to translated target adapter IDs by adapter namespace;
- `require_complete` and `ignore_paths` reserve a strict coverage-check surface.

The dictionary source key is the implicit expected base. A dictionary entry that no longer matches any extracted source text, stable deck path, or adapter ID is stale and composition fails. An addition fails if the target path is no longer blank.

CrowdAnki export renders variables before writing adapter output, so distributable decks still contain plain Anki-compatible text/HTML.

## Rationale

This keeps template structure in one place while allowing each language overlay to translate phrase values and note text. It also keeps translations readable as source text next to translated text without forcing per-note/per-field boilerplate.

Variable-specific translations handle cases where the same English phrase needs different target text or punctuation in different template contexts.

## Implications

- Translation overlays can be much smaller and easier to review.
- Template updates in the base deck flow through all translated targets when the phrase variables stay stable.
- Stale translation entries fail composition instead of silently doing nothing.
- Import/export parity remains based on rendered deck semantics, not source variable syntax.
- Ambiguous source strings can be translated with path-scoped `changes` entries.
- Empty source fields can be filled with `additions` entries that fail if the base later adds content.
