---
title: Replace dotted-string deck paths with a typed DeckPath
priority: medium
---

## Goal

Deck-internal addresses ("field `capital` of note `germany`") are represented by a typed `DeckPath` enum with one parser and one printer, instead of `format!`-assembled dotted strings re-parsed by multiple independent hand-written matchers. Serialized YAML forms stay byte-identical.

## Problem

In `crates/brain-brew-core/src/lib.rs`, paths like `notes.germany.fields.capital` are built with scattered `format!` calls and re-parsed by at least three independent parsers:

- `note_field_path_parts` (~:696) — splits and pattern-matches segments
- `note_id_from_translation_path` (~:1353) — a second parser of the same mini-language for the translation subsystem
- `context_parent_candidates` (~:2251) — string-slices dotted prefixes to find contextual-dictionary parents

Costs:

- The compiler can't check any of it: a typo'd `format!` segment produces a path that never matches — a silent miss (translation stops applying, coverage never resolves), not an error.
- The path grammar exists only as folklore across the parsers, and nothing keeps them in agreement.
- Escaping is undefined: a `.` inside a note/field StableId (nothing forbids it; IDs are human-authored) mis-splits in every parser. Latent today because UG's IDs are dot-free, but it is a data-dependent correctness hole.

## Process

TDD / red-green-refactor:

1. First pin the grammar: `FromStr`/`Display` round-trip tests for every legal path shape currently in use (enumerate them by auditing all construction and parse sites), plus hostile cases — unknown segment kinds, wrong arity, empty segments, and IDs containing `.`.
2. The dotted-ID case forces the design decision below; make it explicitly, then implement `DeckPath` (green).
3. Migrate call sites: construction `format!`s → constructors; hand parsers → `FromStr` + match. Refactor until no hand-rolled dotted-string parsing of deck paths remains in core.

## Acceptance Criteria

- A `DeckPath` enum in core (variants for each path shape actually used: note field, note-model template, etc.) with `Display` emitting the exact current dotted syntax and `FromStr` as the single parser.
- `note_field_path_parts`, `note_id_from_translation_path`, and the prefix-walking in `context_parent_candidates` are expressed via `DeckPath` (deleted or reduced to thin wrappers); no remaining `format!` construction of deck paths outside `Display`.
- Dots-in-IDs decision made and enforced: EITHER reject `.` in StableIds at validation time (preferred if UG data allows — it does today) OR define and test escaping. Document the choice in the code.
- All existing serialized output is byte-identical: composing the full `fixtures/ultimate-geography` deck and formatting all fixture YAML produces identical bytes before and after.
- `cargo test --workspace` passes; existing tests unchanged except where they construct paths as raw strings internally (may switch to constructors).

## Design Decisions

- Internal type change only — the on-disk dotted syntax is a stable format and does not change.
- Sequence AFTER `0100-split-brain-brew-core-lib-into-submodules.md` (the type belongs in the `model` module) and after the translation-resolver unification, which touches the same parsing sites.
