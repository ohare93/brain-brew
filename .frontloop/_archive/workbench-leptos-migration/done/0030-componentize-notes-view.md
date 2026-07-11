---
title: Componentize the notes view — nav, detail, contenteditable fields, card previews, media
priority: high
---

## Goal

Rewrite the notes-view interior as Leptos components, deleting the corresponding legacy string-HTML builders and event closures: note navigation (windowed `.note-navigation-row`, `note-navigation-more`, `navigation-page-summary`), single note detail (`.note-card`, `#note-detail-panel`, `.field-editor` rows), contenteditable field editors (target-translation + source-field, `set_contenteditable`, ids `card-translation-input-*` / `card-source-input-*`), Anki card HTML previews (`note_html` and friends), and media rendering. Staged-edit writes go through a new `staging` module (signals backed by localStorage) preserving the existing key schema exactly. Migrate `NOTE_DETAIL_GENERATION` from `thread_local!` to a signal/`StoredValue` request token with identical drop-stale semantics. Apply box, new-language panel, and multi-pane comparison stay legacy (0040) — keep their publish paths working against the new structure.

Depends on: 0020.

## Delegation

- Agentleman Run `agm-run-20260706155534-tel4o2` (workspace `wb-leptos-0030`), delegated 2026-07-06 via `agm run` + session-scoped `jjw`→`ajj` shim. Checks: `devenv shell test` / `workbench-ui-build` / `e2e`.
- On done: Fable judge gate, then `ajj stack wb-leptos-0030 --repo … --yes` + `update-stale`.

## Outcome (2026-07-06): DONE, judged ACCEPT (after 1 iterate), integrated

Notes-view interior componentized to Leptos; new `staging.rs` (signals + localStorage, exact key schema); `NOTE_DETAIL_GENERATION`→`NOTE_DETAIL_TOKEN_SIGNAL`; contenteditable keyed per note+field, never written back to the focused input; 9 legacy note-view functions deleted. Files: `lib.rs` + new `staging.rs`.

**Iterate:** first Fable pass caught a silent bug — new staging added `context_path` to notes-view contextual edits (legacy handlers never did), which would drift Apply's YAML (field-path context vs consolidated note-level). Fixed via `agm continue` (change `luzywtovqzxo` "preserve note contextual staging shape"): notes view now stages `{kind,path,source,value,mode}`; card/source-string views keep their own legitimate context_path. Re-judged ACCEPT against original criteria. 13/13 e2e post-fix.

Integrated as changes `xwznzyntnrnk` (migrate) + `luzywtovqzxo` (fix). Undo: `jj op restore a0225891d131`.

## Critical constraints

- **contenteditable + reactivity**: never reactively bind element text to the same signal the `input` event writes — re-rendering a focused contenteditable resets the cursor and breaks the typing E2E. Pattern: render initial content once per (note, field) key (key the element on note/field identity so navigation recreates it); handle `on:input` by reading `event_target` textContent into the staging signal; never write that signal back into the focused element. Verify manually via `devenv shell workbench-ui-watch` that typing mid-word keeps the cursor.
- **Media**: `<img>` `src` must be exactly the `/api/media/<path>` URLs the server emits today (rewriting logic ~lib.rs:3801 — port, don't reinterpret). E2E fetches each `src` asserting HTTP status + content-type and checks natural dimensions; a Trunk-mangled/relative URL fails.
- **Row budgets**: preserve windowing — `.note-navigation-row` count and `#note-detail-panel .field-editor tbody tr` count within `WORKBENCH_NAVIGATION_ROW_BUDGET`/`WORKBENCH_DETAIL_ROW_BUDGET`; at most one `.note-card`. Do not render full lists and hide with CSS.
- **localStorage**: staging module reads/writes the exact keys the legacy helpers produce (prefixes + `translation::`/`source::`) — port key-derivation verbatim; E2E refreshes the page expecting staged edits to survive, and the (still-legacy) apply flow reads these keys.
- Preserve stale-note-detail: selecting note B while note A's detail fetch is in flight drops A's response.

## Acceptance Criteria

- All gates green; **13/13** E2E — especially `workbench_app_shell_loads_workspace_metadata`, `workbench_ignores_stale_language_reload_responses`, `workbench_edits_target_translation_persists_refresh_and_applies_yaml`, `workbench_edits_source_field_persists_refresh_and_creates_stale_translation`, and the three `workbench_ultimate_geography_*` / `workbench_loads_ug_like_repeated_source_smoke_path` (media + manifest + row budgets).
- Legacy note-view HTML builders and their `Closure` wiring deleted, not shadowed.
- Zero E2E test edits without explicit reported justification.
- Report: how contenteditable is keyed; how the generation token replaced `NOTE_DETAIL_GENERATION`.
