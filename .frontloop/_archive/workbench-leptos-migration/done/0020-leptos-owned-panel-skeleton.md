---
title: Leptos-owned panel skeleton — view switcher, view sections, probe, error surface
priority: high
---

## Goal

Make Leptos own the structural DOM the legacy code rebuilds wholesale via `set_inner_html`: `#workbench-dom-panel`, the `#workbench-view-switch` nav and its `.workbench-view-button`s, the four `.workbench-view[data-view]` sections (`notes`/`cards`/`source-strings`/`metadata`, ids `view-panel-notes` etc., lib.rs ~368–427; switch builder ~438), workspace probe updates, and the top-level error display (`publish_note_pivot_error` / `.workbench-error`). View interiors stay legacy: each section exposes a stable inner container that the existing `publish_*` functions render into.

Depends on: 0010 (Leptos bootstrap in place).

## Work items

1. Introduce the reactive spine: signals for `selected_view` (Notes/Cards/SourceStrings/Metadata), workspace summary, top-level error/status. Replace `WORKBENCH_SELECTION_GENERATION` consumers only where they guard *view switching*; leave per-detail generation counters for later stages. Preserve stale-selection semantics exactly — a late response from a superseded selection is dropped, not rendered (E2E `workbench_ignores_stale_language_reload_responses` asserts the stale node is absent from `#workbench-dom-panel`).
2. Re-scope the legacy publish functions minimally so each renders only its view's interior into its section container instead of regenerating the whole panel + switcher. Do not change the HTML they emit inside sections.
3. Keep view activation lazy: switching to a view triggers that view's legacy fetch/publish on first activation; non-active views must not fetch (E2E instruments `window.fetch`, `assert_no_secondary_pivot_fetches`). Preserve `.active`/`[hidden]`/`aria-current="page"` on sections and buttons.
4. The switcher must render OUTSIDE all `.workbench-view` sections (E2E checks the nested case finds nothing).
5. Move probe updates to a Leptos effect if convenient, keeping identical `data-status` transitions/text; keeping the web_sys impl is also fine.
6. Reuse `static/workbench.css` as-is; add no stylesheet unless a class is genuinely missing (report additions).

## Delegation

- Agentleman Run `agm-run-20260706152604-7rinyr` (workspace `wb-leptos-0020`), delegated 2026-07-06 via `agm run` + a session-scoped `jjw`→`ajj` shim (the MCP delegate tool is broken by the rename — see [[agentleman-integration-via-ajj]]). Checks: `devenv shell test` / `workbench-ui-build` / `e2e`.
- On done: Fable judge gate, then integrate with `ajj stack wb-leptos-0020 --repo … --yes` + `jj workspace update-stale`.

## Outcome (2026-07-06): DONE, judged ACCEPT, integrated

Fable judge **ACCEPT**. Leptos now owns `#workbench-dom-panel`, `#workbench-view-switch` + `.workbench-view-button`s, the four `.workbench-view` sections (stable interior containers `#note-pivot-panel`/`#card-pivot-panel`/`#source-string-pivot-panel`/`#optional-metadata-panel`), and a signal-driven `.workbench-error` surface; legacy `publish_*` fill interiors with byte-identical HTML. 13/13 e2e (transcript-evidenced). Single file changed (`lib.rs`, +337/−96). Probe intentionally stays the static `#brainbrew-workbench-e2e` div in index.html (index.html is no-go). Integrated as jj change `skpqtnzu` (commit `66c…`). Undo: `jj op restore f5dc34a6e268`.

Carry-forward: ~60 lines of dead view-switch code (`workbench_view_switch_html`, `register_view_switch_handlers`, `attach_view_switch_handler`, `activate_workbench_view`) left under `#[allow(dead_code)]` — delete in 0060.

## Acceptance Criteria

- All validation gates green; **13/13** E2E; zero E2E test edits (if the Leptos DOM legitimately cannot reproduce a pinned selector, STOP and report rather than editing the test).
- Diff shows the whole-panel `set_inner_html` rebuild replaced by a Leptos-rendered skeleton + per-section legacy publishes.
- Report which publish functions were re-scoped.
