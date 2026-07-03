---
title: Add HTML/CSS content validation to verify
priority: medium
---

## Goal

`verify` validates the HTML and CSS content a deck ships to Anki — template fragments, deck descriptions, styling — natively in Rust, wherever that content lives (inline or `!include`d). Brain Brew is an Anki deck tool; HTML is always part of the product, so the tool owns its validation. UG's Python side-script becomes deletable.

## Problem

`verify` checks YAML structure, translations, and media, but never looks inside template/description/styling content. A template with a mismatched tag or a stylesheet with an unbalanced brace passes verify and export, then fails at render time in Anki on a user's device. UG plugged the hole with `scripts/check-source-content.py` (~150 lines of Python: HTML fragment well-formedness via `html.parser`, CSS brace/comment/string balance), wired into `integrity-check.yml:26` and CONTRIBUTING:

- Protection is UG-only and layout-coupled: the script scans three hardcoded directories (`descriptions/`, `templates/`, `styles/`), so other consumers get nothing and UG's INLINE HTML (field values, non-externalized content) is invisible to it.
- It drags a Python runtime into otherwise pure-Rust/Nix CI — the out-of-band-script pattern this rewrite exists to end.
- It runs too late: CI-time only. The workbench will happily APPLY a malformed template edit today.

## Process

Parity first, then port:

1. Capture the Python script's actual tolerances as a test corpus BEFORE writing Rust: run it against UG's real templates/descriptions/styles plus deliberately broken variants (mismatched tag, unclosed tag, stray `</div>`, unbalanced brace, unterminated comment/string in CSS, `{{Field}}` and `{{cloze:...}}` mustache, void elements like `<br>`/`<img>`, HTML entities). Record accept/reject for each — the Rust check must not silently reject content the script blessed or accept what it caught.
2. Implement the checks in Rust against that corpus (red-green), as a pure function over content strings in core or formats — no filesystem knowledge.
3. Wire into `verify` over the COMPOSED deck (covers inline and included content uniformly, per target), and expose the same function to the workbench apply path so malformed template edits are rejected at write time.

## Acceptance Criteria

- A `verify` sub-check validates, for every target: note-model templates (question/answer), deck descriptions, and styling — reporting file/path-precise errors (which template, which target, what's wrong).
- Checks match the side-script's lightweight ambition (this is NOT a spec-grade HTML validator): fragment tag balance tolerant of Anki mustache syntax and void elements; CSS brace/comment/string balance. No heavyweight parser dependency (no html5ever) unless the parity corpus forces it.
- Hand-rolled in Rust, zero Python anywhere; pure function reusable by the workbench apply path (wiring apply-side rejection may land here or ride the apply-atomicity task — either way the function is shared, not duplicated).
- The parity corpus lives in the repo as table-driven tests.
- An escape hatch exists for false positives (per-check severity or an opt-out flag), documented — Anki tolerates a lot of sloppy HTML; the check must not block a deck Anki renders fine without a way out.
- `cargo test --workspace` passes; verify docs updated.

## Design Decisions

- Scope creep guard: no CSS property validation, no HTML attribute/semantics linting, no external references — structural well-formedness only, matching what the Python script proved useful.
- UG-side deletion of the script is a separate task in the UG repo, triggered on the next Brain Brew publish.
