---
title: Componentize card, source-string, and metadata views
priority: high
---

## Goal

Rewrite the three remaining view interiors as Leptos components and delete their legacy counterparts:

- **Card view**: `publish_card_list_panel`, `publish_card_detail_panel`, `publish_card_pivot_panel`; `#card-pivot-panel`, `.card-row`, card filters; endpoints `card-list`/`card-pivot`.
- **Source-string view**: `publish_source_string_list_panel`, `_detail_panel`, `_pivot_panel`; `#source-string-pivot-panel`, `.source-string-row`, `source-string-contextual-input-*` contenteditable; endpoints `source-string-list`/`source-string-pivot`.
- **Metadata view**: `publish_optional_metadata_panel`, the optional-metadata checklist; endpoints `metadata`/`metadata-list`.

Migrate `CARD_DETAIL_GENERATION` and `SOURCE_STRING_DETAIL_GENERATION` to the same signal-token pattern from 0030. Card/source-string field editing goes through the shared staging module (same key schema). Follow the same contenteditable, row-budget, and lazy-activation constraints as 0030 (these views must not fetch until first activated; row counts within `WORKBENCH_NAVIGATION_ROW_BUDGET`).

Also componentize the **global language/target/overlay controls** (`#workbench-global-controls`, filled today by legacy `set_inner_html` at lib.rs ~1633) so that after 0050 the only remaining raw-HTML injection is trusted card/answer preview bodies (rendered via Leptos `inner_html`), leaving 0060 a true purge. If including the global controls makes this stage too large, complete the three views first and report so the controls can be split out.

Depends on: 0030 (staging module + token pattern), 0040.

## Delegation

- Agentleman Run `agm-run-20260706173111-tl5vqe` (workspace `wb-leptos-0050`), delegated 2026-07-06 via `agm run` + session-scoped `jjw`→`ajj` shim. Checks: `devenv shell test` / `workbench-ui-build` / `e2e`.
- Note: card & source-string staged translations LEGITIMATELY include `context_path` (unlike the notes view) — do NOT strip it. On done: Fable judge, then `ajj stack` + `update-stale`.

## Outcome (2026-07-06): DONE, judged ACCEPT, integrated

Card, source-string, metadata view interiors AND global controls componentized to Leptos. `CARD_DETAIL_GENERATION`/`SOURCE_STRING_DETAIL_GENERATION` → `CARD_/SOURCE_STRING_DETAIL_TOKEN_SIGNAL` RwSignal tokens. **All `set_inner_html` drained** — 5 remaining `inner_html` sites are trusted deck-authored preview bodies only. `context_path` split correct (card/source-string keep it, notes don't; `staging.rs` byte-unchanged). Lazy activation + pagination preserved. Only `lib.rs` changed; 13/13 e2e (delegate self-corrected a transient 12/1 mid-run). Integrated as change `okolynrzwyoo` "port remaining pivots to leptos". Undo: `jj op restore cc42438c8867`.

The entire Workbench UI now renders from Leptos. 0060 is a true purge.

## Acceptance Criteria

- All gates green; **13/13** E2E — especially `workbench_card_pivot_navigates_and_edits_card_field`, `workbench_source_string_pivot_stages_direct_translation`, `workbench_optional_metadata_checklist_edits_separately`, plus re-run confidence on the notes suite (shared staging module now has three consumers).
- All three views' legacy builders and closures deleted.
- Zero E2E test edits without reported justification.

## Note

Largest remaining chunk. Views are structurally similar (list/detail/pivot each) and share the established staging module + token pattern. If it stalls, complete and validate view-by-view and report honestly rather than delivering a broken mixture — the orchestrator can `agm continue` the remainder.
