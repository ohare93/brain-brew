---
title: Purge legacy remnants, trim dependencies, embed refresh, full CI
priority: medium
---

## Outcome (2026-07-06): DONE, judged ACCEPT, integrated — EPIC COMPLETE

All `thread_local!` removed (selection/note-pivot/detail generations moved into a `WorkbenchSignals` `RwSignal` holder in a `OnceLock`; drop-stale guard + generation semantics verified preserved). `wasm-bindgen-futures` dropped (all 12 `spawn_local` sites → `leptos::task::spawn_local`); `wasm-bindgen`/`gloo-net`/`web-sys` kept as still-used. Dead CSS (`.card-filters`, `.error`) removed. `inner_html` only on trusted deck-preview bodies. `staging.rs` byte-unchanged. `cli.md` + `workbench.md` updated to Leptos; ADRs + research doc left historical. Embedded assets regenerated. **Full `devenv shell ci` green, 13/13 e2e.** Integrated as change `rzvlzorrkynm` "purge leptos migration leftovers". Undo: `jj op restore 1ad53fae7690`.

**Follow-up for the lead:** supersede ADR-0011 (which committed to Iced) with a new ADR recording the Leptos decision. Delegate recommended it but (correctly) did not write it.

## Goal

Finish the migration. Remove every remaining legacy artifact: unused `thread_local!` cells, `set_inner_html`-based helpers, dead `Closure` wiring, orphaned localStorage helper duplicates, and `#[cfg]` scaffolding left from the strangler phases. Trim `Cargo.toml`: drop `web-sys` features no longer referenced (audit each of Document/Element/Event/EventTarget/HtmlInputElement/HtmlSelectElement/Node/NodeList/Storage/Window against actual usage — Leptos re-exports much of this); drop `wasm-bindgen`/`wasm-bindgen-futures` as direct deps if nothing references them directly; keep `gloo-net` only if still the fetch layer. Sanity-pass `static/workbench.css` for selectors no longer matching any rendered DOM — remove only provably dead rules, leave anything E2E references. Update stale in-crate comments/docs still describing Iced or `set_inner_html`.

Depends on: 0050 (all views componentized).

## Delegation

- Agentleman Run `agm-run-20260706175839-nvgybb` (workspace `wb-leptos-0060`), delegated 2026-07-06 via `agm run` + session-scoped `jjw`→`ajj` shim. Checks: `devenv shell test` / `workbench-ui-build` / `e2e`.
- Sanctioned out-of-crate writes: regenerate `crates/brain-brew-cli/assets/workbench`; factual Iced→Leptos wording in `documentation/docs/reference/cli.md` + `workbench.md` only (ADRs + research doc left historical; delegate recommends ADR-0011 supersession for the lead). On done: Fable judge, then `ajj stack` + `update-stale`.

Known carry-forward from 0020: delete the dead view-switch code left under `#[allow(dead_code)]` — `workbench_view_switch_html`, `register_view_switch_handlers`, `attach_view_switch_handler`, `activate_workbench_view` (~60 lines). Also audit for additive-but-inert DOM surface introduced during componentization (`data-app-status`, `data-workspace-loaded`, `#workbench-global-controls`, `.workbench-view-interior` with no CSS) — adopt or remove.

## Narrow out-of-crate exception

This is the one stage allowed to touch files outside the UI crate, narrowly: regenerate `crates/brain-brew-cli/assets/workbench` via `devenv shell workbench-ui-embed`; and if repo docs/ADRs under `documentation/` describe the workbench frontend as Iced, update those descriptions factually. **No new ADR** — the lead owns the ADR record (ADR-0011 supersession); report what you found if an ADR change seems needed rather than writing one.

## Acceptance Criteria

- `rg -in "iced|set_inner_html" crates/brain-brew-workbench-ui/src/` returns nothing — except any deliberate, justified raw-HTML injection for trusted server-provided card-template previews (if Anki preview rendering legitimately needs Leptos `inner_html`, that is expected; report each site).
- No `thread_local!` in the crate.
- Full `devenv shell ci` green (fmt:check + workspace tests + clippy `-D warnings` + complete **13/13** e2e).
- Embedded assets regenerated and included in the payload.
- `tests/workspace_summary.rs` still unmodified and green.
- Report: final dependency list, deleted-line count, CSS rules removed, doc files touched.
