---
title: Migrate Workbench UI to a declarative state-driven architecture
priority: medium
---

## Goal

Replace the hand-rolled HTML-string/`innerHTML` layer in `crates/brain-brew-workbench-ui` with a real state→view architecture, so the UI renders deterministically from typed state, as ADR-0011 promised. No one uses the workbench yet, so breaking changes are acceptable; correctness and determinism over compatibility.

## Problem

ADR-0011 commits to an Iced/WASM state architecture, but the Iced app in `crates/brain-brew-workbench-ui/src/lib.rs` is a vestigial three-panel shell (view/update handle ~3 messages, ~lines 131-207). The entire real workbench (note list/pivot, card detail, source-string editing, staged edits, Apply) is built by `format!`-assembling HTML strings and injecting them via `web_sys` `innerHTML` with manually managed `Closure` event handlers (`publish_note_pivot_panel` ~:271, `note_html` ~:658, `card_detail_html` ~:1133). Concrete costs:

- `note_html` is one `format!` with ~46 positional arguments (~lines 717-761); several builders share this shape. Positional drift is silent and the compiler cannot help.
- Two hand-rolled HTML escapers disagree: server `html_attribute_escape` escapes `'` (`workbench.rs` ~:3557), UI `escape_html` does not (~lib.rs:4754) — an injection seam in single-quoted attributes.
- E2E tests can only assert on raw DOM markup, so any markup change breaks them (against ADR-0014 intent).
- Stale artifact: UI status text says "loaded from /api/workbench/note-pivot" (~lib.rs:115) while code calls `/note-list` (~:4815).

## Acceptance Criteria

- All UI panels render from typed Rust state through the chosen framework's view layer; zero `innerHTML` string injection and zero hand-built HTML `format!` strings remain in `brain-brew-workbench-ui`.
- All event handling goes through the framework's message/event system; no manually managed `web_sys::Closure` listeners remain.
- Escaping is handled by the framework (or one shared audited escaper if any raw HTML rendering of deck content is still required, e.g. card previews — that path must be explicit and documented).
- ADR-0011 is superseded/amended by a new ADR recording the chosen framework and why.
- The existing E2E suite (`brain-brew-workbench-e2e`) passes, updated where markup assertions must change; prefer data-testid/semantic hooks over structural selectors while updating.
- Workbench API surface in the CLI crate is unchanged (this is a UI-crate migration).

## Design Decisions

- Phase 0 spike (timeboxed): evaluate whether current Iced WASM/DOM support can express the workbench (forms, text editing, lists, HTML card preview). If yes, fulfill ADR-0011 with Iced. If not, pick a declarative Rust WASM framework (Leptos or Dioxus are the expected candidates) and record the choice in the superseding ADR.
- Migrate panel-by-panel (note list → note pivot → card detail → apply flow), keeping the app shippable and E2E green at each step, rather than a big-bang rewrite.
- Card/answer HTML previews render deck-authored HTML by design; sandbox them (dedicated container, explicit trusted-content boundary) rather than letting deck content share the app's DOM context.

## Implementation Notes

- Server endpoints already return JSON (ADR-0015 list/detail shape); the UI migration should not need server changes beyond possibly retiring the legacy full-pivot scaffolding routes.
- Fix the stale status-string endpoint mention as part of the first migrated panel.
- This task is large; if it stalls, split into a dedicated epic with the spike as its first task.
