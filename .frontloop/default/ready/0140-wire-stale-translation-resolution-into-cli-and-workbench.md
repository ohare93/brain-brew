---
title: Wire stale-translation resolution into the CLI and Workbench
priority: high
---

## Goal

A stale translation record can be resolved through the tool — "confirm still correct" (promote the existing translation under the new source text) or "replace" (provide a new translation, retire the stale record) — in both the translations CLI and the Workbench, completing the ADR-0013 workflow. Hand-editing overlay YAML stops being the only resolution path.

## Problem

Detection is thoroughly built: compose flags stale records, verify warns, the translations CLI counts/color-codes five stale coverage categories. But resolution exists nowhere:

- `TranslationDictionary::resolve_stale_translation(old_source, new_source, context)` (`crates/brain-brew-core/src/lib.rs` ~:3996) — the purpose-built promote mutation, tested in `overlay_compose.rs` (~:490, :496) — has ZERO non-test callers.
- The Workbench, which ADR-0013 explicitly casts as the resolve surface, has no stale-resolution endpoint at all.
- The CLI apply flow detects stale rows and skips them (`translations.rs` ~:1096, "no safe automatic rewrite is applied, skipping") — correct fail-safe, but the guided flow dead-ends at exactly the records needing human judgment.

Cost for UG: 16 languages × every English wording tweak = a steady drip of stale records, each costing a translator a YAML-archaeology session. The feature's value is capped by its resolution ergonomics.

## Process

TDD / red-green-refactor. Failing tests first, then implement, then refactor both surfaces onto shared plumbing:

1. Core-adjacent: resolving a stale record via the CLI path rewrites the overlay dictionary correctly for both verbs — "confirm" promotes the existing translation under the new source text and removes the stale record; "replace" installs the new translation and removes the stale record. Assert exact canonical YAML output (goes through the canonical emitter, respects include preservation once that lands).
2. Resolution of a contextual stale record preserves its context key (the `Some("notes.note.finland")`-style case already covered in core tests must survive the CLI plumbing).
3. Batch confirm: an English change touching many notes with unchanged meaning can be confirmed in one command invocation; test a multi-record fixture.
4. Refusals: resolving a record that is not stale, or whose new source text doesn't match the current base, is a precise error, not a silent no-op (fail closed per ADR-0010).
5. Workbench: a stale record in the selected translation context is resolvable via an API endpoint for each verb; the write goes through the (queued) atomic-apply machinery and bumps the freshness generation; the next context fetch shows the record resolved.

## Acceptance Criteria

- `translations` CLI gains a resolve flow (subcommand or flags — match the existing CLI's conventions) offering confirm/replace, single-record and batch forms; the skip-warning path (~:1096) points users at it.
- Workbench API exposes stale resolution for the selected translation context (both verbs); UI affordance may be minimal (two actions on a stale row) and should land on whichever UI architecture is current — coordinate with `0130-migrate-workbench-ui-to-declarative-state-driven-architecture.md` rather than building elaborate legacy-path UI.
- Both surfaces are thin adapters over `resolve_stale_translation` (and the existing overlay write/serialize plumbing) — no second resolution implementation.
- All dictionary writes produce canonical YAML byte-identical to what `fmt` would produce.
- `cargo test --workspace` passes; E2E extended with one stale-resolve happy path if the workbench UI affordance lands here.

## Design Decisions

- Sequencing: the CLI half has no dependencies — do it first. The Workbench half should follow the apply-atomicity task (`0120-make-workbench-apply-atomic-all-or-nothing.md`) and ride the UI migration rather than racing it.
- "Confirm" must NOT silently rewrite translations whose meaning plausibly changed — it is an explicit human verb, never an automatic rewrite (the current skip behavior stays the default for non-interactive apply).
