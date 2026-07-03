---
title: Unify translation resolution into a single resolver
priority: high
---

## Goal

One implementation of the translation decision procedure (direct / contextual / no_change / stale / missing), consumed by both the apply path and the coverage path, replacing the current hand-synced parallel copies.

## Problem

In `crates/brain-brew-core/src/lib.rs` the same ~100-line resolution decision tree is written out longhand at least four times:

- `TranslationApplyContext::translate_string` (~:1909-2037) — mutating apply path used by compose.
- `TranslationCoverageBuilder::record_string` (~:924-1050) — read-only classification used by verify/reports.
- `translate_optional_string` (~:2039) and `record_optional_string` (~:1052) — near-identical nullable-field twins.
- `has_explicit_string_entry` is duplicated verbatim in both structs (~:894-922 and ~:1879-1907).

Nothing enforces agreement between the copies. Divergence means `verify` reports coverage that differs from what compose actually produces — breaking the tool's core promise that what verify says is what ships.

## Acceptance Criteria

- A single resolver function/type takes (source string, path, dictionary, options) and returns a `TranslationOutcome` enum (e.g. `Direct(value)` / `Contextual { value, context }` / `NoChange` / `Stale(record)` / `Missing`).
- `TranslationApplyContext` and `TranslationCoverageBuilder` are thin consumers mapping outcomes to mutations/errors and report entries respectively; the `_optional` variants collapse into `Option` handling at call sites; `has_explicit_string_entry` exists exactly once.
- Behavior-preserving: all existing tests pass unchanged (33 `overlay_compose` tests, translation CLI tests, UG fixture tests). If the parallel copies turn out to have ALREADY drifted (tests or UG fixture composition reveal a semantic difference between apply and coverage), STOP and report the divergence for a human decision rather than silently picking one side.
- Composing the full ultimate-geography fixture (`fixtures/ultimate-geography`) produces byte-identical output before and after the refactor (capture a before snapshot as evidence).
- `cargo test --workspace` passes.

## Design Decisions

- Pure refactor — no semantic changes, no new features. Any tempting behavior fix found along the way becomes a separate task.
- Resolver should be a free function or small struct in core, not a trait hierarchy; keep it as boring as possible.

## Implementation Notes

- The translation subsystem spans roughly lib.rs:722-2331; this refactor is a natural precursor to splitting lib.rs into submodules (separate task) — do this one first so the module split moves one resolver, not four copies.
- Guardrails are strong: work red-green against the existing suites; diff the resolution behavior across the 16 UG language overlays as an end-to-end check.
