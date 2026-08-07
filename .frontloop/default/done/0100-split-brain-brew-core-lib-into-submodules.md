---
title: Split brain-brew-core lib.rs into submodules
priority: medium
---

## Goal

`crates/brain-brew-core/src/lib.rs` (currently one flat ~4,662-line module) is split into focused submodules with the public API unchanged, so the crate's internal boundaries are enforced by the module system instead of by discipline.

## Problem

The entire core crate — deck model, overlay compose, the translation subsystem (~1,600 lines, roughly :722-2331), structured messages, validation, variable rendering — lives in a single flat `lib.rs`:

- No enforced boundaries: everything can reach everything's private items; translation code can poke at compose internals and vice versa. Several already-filed duplications (twin translation resolvers, twin `has_explicit_string_entry`) survived partly because a 4,662-line file hides the same logic existing twice a thousand lines apart.
- Zero unit tests in core: all coverage is black-box in `tests/` (good, but the layout discourages small private-function tests next to a `mod translation`).
- Review friction: every core change diffs the same file; "did this translation change touch compose?" means scrolling, not reading a module path.

The formats crate already splits by codec — this file is the outlier, not the house style.

## Sequencing (IMPORTANT)

Do this AFTER these already-filed tasks land, so the split moves one resolver instead of four copies and the same lines aren't churned twice:

1. `0050-unify-translation-resolution-into-single-resolver.md`
2. `0090-dedupe-verbatim-copy-paste-sites.md` (at least item 3, the `resolve_structured_messages_with_*_errors` merge)

## Acceptance Criteria

- `lib.rs` becomes a thin root: module declarations + `pub use` re-exports. Suggested split (adjust to the natural seams found while moving): `model`, `compose`, `translation`, `messages`, `validate`.
- Pure code motion: no function bodies change, no visibility widens beyond what compilation requires (prefer `pub(crate)` over `pub` for items that were previously private-in-module).
- Public API of the crate is unchanged — downstream crates (`brain-brew-formats`, `brain-brew-cli`) compile without source changes to their imports (re-exports preserve paths).
- `cargo test --workspace` passes with zero test changes.
- Composing the full `fixtures/ultimate-geography` deck produces byte-identical output before and after (trivially true for code motion; capture as evidence anyway).

## Design Decisions

- No behavior changes, no dedup, no renames beyond module paths — any improvement noticed while moving code becomes a separate task.
- Keep it to one level of modules; don't design a deep hierarchy speculatively.
