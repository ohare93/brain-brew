---
title: Componentize notes-view workflows — apply box, apply preview, new-language, multi-pane
priority: high
---

## Goal

Rewrite the remaining notes-view surfaces as Leptos components, deleting their legacy builders/closures: the apply box (`.apply-box`; POST `/api/workbench/apply-preview` + `/api/workbench/apply`, lib.rs ~4454), the new-language panel (`.new-language-panel`; GET `new-language-preview`, POST `new-language`, ~3440), and the multi-pane secondary comparison (`publish_secondary_target_pane`, `.pane-layout-panel`, GET `comparison-pane`). These read staged edits from the 0030 staging module and clear/refresh state after apply exactly as the legacy flow does (post-apply refresh must NOT start a new user selection — see comment ~lib.rs:4787; preserve that).

Depends on: 0030 (staging module + note components).

## Delegation

- Agentleman Run `agm-run-20260706170848-1la4br` (workspace `wb-leptos-0040`), delegated 2026-07-06 via `agm run` + session-scoped `jjw`→`ajj` shim. Checks: `devenv shell test` / `workbench-ui-build` / `e2e`.
- On done: Fable judge gate (watch apply/preview/new-language request-JSON parity — highest silent-drift risk), then `ajj stack wb-leptos-0040 --repo … --yes` + `update-stale`.

## Outcome (2026-07-06): DONE, judged ACCEPT, integrated

Apply box, apply-preview, new-language scaffold, and multi-pane comparison componentized to Leptos. Apply/apply-preview/new-language request JSON verified byte-identical to legacy builders (Fable checked field-for-field — the highest silent-drift risk); staged edits collected via `staging::collect_staged_edits_for_prefixes(active_storage_prefixes(pivot))` (active pivot + secondary panes, scope unchanged); post-apply clear/refresh guard preserved. 17 legacy builders/closures deleted; only `lib.rs` changed; 13/13 e2e. Integrated as change `qulrqznup` "port remaining notes workflow panels to leptos". Undo: `jj op restore 2b691c858d05`.

## Constraints

- Request/response payloads to apply/apply-preview/new-language stay identical to what the server expects — port the JSON construction, don't redesign it.
- After a successful apply, staged localStorage entries are cleared the same way.
- Grouped cross-file apply and mixed source+target flows are the most ordering-sensitive: staged source edits must take effect before dependent target rows are computed client-side — port the `effective_source` overlay logic faithfully (~lib.rs:1705/1817/3972/4097).

## Acceptance Criteria

- All gates green; **13/13** E2E — especially `workbench_new_language_scaffold_creates_editable_language`, `workbench_multi_pane_layout_applies_grouped_changes_across_files`, `workbench_mixed_source_and_target_browser_apply_uses_new_source`, and re-confirm `workbench_edits_target_translation_persists_refresh_and_applies_yaml` (apply path now componentized).
- Legacy builders for these panels deleted.
- Zero E2E test edits without reported justification.
