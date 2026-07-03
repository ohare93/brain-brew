---
title: Dedupe scattered verbatim copy-paste sites
priority: low
---

## Goal

One behavior-preserving sweep that removes the remaining verbatim code duplication, so each of these pieces of logic exists exactly once. (The two load-bearing duplications — translation resolution and YAML scalar emission — have their own dedicated tasks and are OUT of scope here.)

## Problem

Four independent copy-paste sites, each a divergence waiting to happen:

1. **`compose_lenient_translation_overlay`** — ~63 lines byte-identical in `crates/brain-brew-cli/src/commands/workbench.rs` ~:4414 and `crates/brain-brew-cli/src/commands/translations.rs` ~:1314. This defines the "compose without failing on missing translations" semantics both the workbench and the translations CLI rely on; a fix to one copy silently misses the other and the two frontends then disagree about current deck state.
2. **`glob_matches`** — byte-identical in `crates/brain-brew-core/src/lib.rs` ~:2316 and `crates/brain-brew-formats/src/crowdanki.rs` ~:480. Also a hand-rolled recursive backtracker with exponential worst case on patterns with multiple `*`. Not exploitable today (patterns are short and user-authored) but there is no reason for two copies or the exponential body.
3. **`resolve_structured_messages_with_{validation,compose}_errors`** — `crates/brain-brew-core/src/lib.rs` ~:508 and ~:533, identical except which error variant they push. Both also take a full `deck.clone()` snapshot, and the pair runs once per overlay during compose — the largest contributor to compose's clone-per-overlay cost.
4. **Stale-translation warning loop** — the same collect-and-format loop appears three times in `crates/brain-brew-cli/src/commands/verify.rs` (~:114-133, ~:163, ~:190). Consolidation only; the risk is wording/behavior drift between verify output paths.

## Acceptance Criteria

- `compose_lenient_translation_overlay` exists exactly once, in a location importable by both `workbench.rs` and `translations.rs` (a shared module in the CLI crate is fine; core is fine too if it fits without dragging CLI concerns in). Note: the workbench composition-cache task also touches this hot path — coordinate if both are in flight.
- `glob_matches` exists exactly once (in core, called or re-exported from formats), and its body is replaced with a linear-time two-pointer match (or the `globset` crate). Behavior pinned by a table-driven test over patterns/inputs covering `*` at start/middle/end, multiple `*`, literal-only, empty pattern, empty input — written BEFORE swapping the body.
- The two `resolve_structured_messages_with_*_errors` functions collapse into one implementation, generic over the error constructor or returning a neutral error type both callers map. If the deck clone can be replaced with borrows of the maps actually needed, do it; if that turns invasive, keep the clone and note it — that half is severable.
- The verify.rs stale-warning loop exists exactly once; the three call sites produce byte-identical output to before (pin with a test or capture before/after on the UG fixture).
- Behavior-preserving throughout: `cargo test --workspace` passes with no test expectation changes (new tests may be added).

## Design Decisions

- Explicitly out of scope: the ~250 `.expect("writing to a string cannot fail")` calls on `write!`-to-String — genuinely infallible, idiomatic, leave them alone.
- No semantic changes anywhere; any tempting behavior fix discovered along the way becomes a separate task.
