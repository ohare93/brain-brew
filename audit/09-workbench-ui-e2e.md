## Review

### Scope and user-flow map

- The requested `plan.md` and `progress.md` were absent, so no plan/progress assumptions could be verified.
- Current UI startup launches `/api/workspace` and the first `/api/workbench/note-list` request in parallel, then auto-loads the first note detail (`crates/brain-brew-workbench-ui/src/lib.rs:106-178,2579-2597`).
- Notes are the default view. Cards, Source Strings, and Metadata are lazy-loaded when their view buttons are opened (`crates/brain-brew-workbench-ui/src/lib.rs:197-289,2942-2970`).
- Note/Card/Source String edits are staged immediately on input into `localStorage`, keyed by language, target, overlay, kind, path, and original source (`crates/brain-brew-workbench-ui/src/staging.rs:41-80,130-139`). Refresh and selected-item navigation restore those drafts.
- Apply Preview and Confirm Apply are global controls below every view. New-language scaffolding and an optional comparison-language pane are also global workflow panels (`crates/brain-brew-workbench-ui/src/lib.rs:290-293,1613-1694`).

### Correct

- The happy paths are broad and currently green: the browser suite exercises real WASM UI startup, visible inline editing with caret preservation, refresh persistence, file mutation, source edits, source strings, metadata, multi-pane writes, lazy views, stale language-list responses, real Ultimate Geography navigation, and media (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:28-343`). `devenv shell e2e` passed all 13 tests.
- Selection and selected-detail requests have generation guards, and the delayed-language E2E test verifies a late list response cannot replace the latest selection (`crates/brain-brew-workbench-ui/src/lib.rs:3096-3175`; `crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:402-493`).
- Note/Card details are single-item mounted, advanced controls are collapsed, secondary pivots are lazy, and the real UG E2E enforces bounded initial DOM counts (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:800-932,1791-1860,2297-2335`).
- Failure artifacts include a screenshot, DOM, server logs, and structured browser/media/caret diagnostics (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:2490-2596`). Polling helpers have replaced fixed sleeps for user assertions (`:1862-1957`).
- Embedded assets have hashed JS/WASM/CSS names and SRI. The release rebuild matched the checked-in assets byte-for-byte via `scripts/check_workbench_ui_embed.sh:1-32`, and that check is in CI (`devenv.nix:47-52,108-115`; `.github/workflows/ci.yml:25-33`).

### Severity-ranked findings

#### P0 — Blocker: stale browser drafts have no file-conflict protection and can overwrite external YAML edits

`/api/workspace` returns fingerprints, but the UI reduces them to a count and discards their values (`crates/brain-brew-workbench-ui/src/lib.rs:2518-2538`). It does not poll again. Preview/Apply requests contain only selection plus edits, with no expected fingerprints or workspace generation (`:1629-1658`). This is the exact hazard ADR-0011 calls out for browser-local drafts (`documentation/docs/reference/decisions/0011-use-a-local-deck-workbench-server-with-iced-wasm-ui.md:35-36,47-50`): a maintainer can stage an edit, change the same overlay in an editor or another process, and later Confirm Apply without any warning.

**Concrete tests to add:** browser E2E that stages a translation, externally changes the same YAML key, then requires Preview and Confirm to show a conflict and leave the external bytes intact; repeat with a file change between Preview and Confirm. Add API CAS tests for overlay, deck, manifest, and included-scalar fingerprints.

#### P1 — Blocker: deck-wide progress is replaced with selected-note-only counts

The list response supplies deck-wide totals and initially renders them (`crates/brain-brew-workbench-ui/src/lib.rs:397-427`). As soon as a note detail mounts, `decorate_note_detail_dom` calls `refresh_progress_from_dom` (`:2275-2287`), which counts only the currently mounted detail rows and overwrites complete/missing/total while retaining the old deck-wide stale count (`:3743-3805`). On UG this can display a handful of fields as the whole deck, potentially with `stale > total`. The real-UG E2E checks only that the phrase “Main note-field progress” exists, not its values (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:807-810`). This contradicts ADR-0015’s deck-wide-total requirement (`documentation/docs/reference/decisions/0015-use-lazy-single-work-item-workbench-editing.md:52-57`).

**Concrete test to add:** on the real UG fixture, capture `/note-list` progress, wait for first detail, navigate and stage one field, and assert the visible/data-attribute totals remain deck-wide while only the relevant aggregate changes.

#### P1 — Blocker: items after the first 50 are unreachable

The server defaults lists to 50 rows, but UI fetches never send `limit` or `offset`. Notes explicitly show a placeholder saying controls are for “the next slice” (`crates/brain-brew-workbench-ui/src/lib.rs:592-596`); Cards and Source Strings only report `Showing rows.len() of total` and provide no page/load-more control (`:1013-1044,1315-1343`). The real UG test only asserts row count is at most 50 (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:889-896,916-923`), so it validates boundedness while missing reachability. This leaves most UG items uneditable and does not fulfill ADR-0015’s paginated-navigation decision (`documentation/docs/reference/decisions/0015-use-lazy-single-work-item-workbench-editing.md:17,50-59`).

**Concrete tests to add:** use the UG fixture to navigate to and edit note/card/source-string item 51+ through visible controls; assert advancing `offset` requests and the DOM budget after every page. Add metadata navigation/detail coverage too.

#### P1 — Blocker: Confirm Apply bypasses Preview, and existing E2E codifies the bypass

Preview and Confirm are independent, always-enabled buttons; Confirm posts directly to `/apply` regardless of preview state (`crates/brain-brew-workbench-ui/src/lib.rs:1629-1689`). This violates ADR-0011’s required validation/preview step and ADR-0015’s preview-then-confirm flow (`documentation/docs/reference/decisions/0011-use-a-local-deck-workbench-server-with-iced-wasm-ui.md:50`; `documentation/docs/reference/decisions/0015-use-lazy-single-work-item-workbench-editing.md:22`). The new-language edit and mixed source/target browser tests deliberately click Confirm without Preview (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:627-646,1408-1428`). Draft changes, selection changes, or file changes also do not invalidate a prior preview.

**Concrete tests to add:** assert Confirm is disabled until a successful preview; mutate a draft after preview and assert Confirm is disabled again; change a watched file after preview and require re-preview/conflict; double-click Preview/Confirm and assert only one request/write occurs.

#### P1 — Staged edits from unmounted scopes disappear from Apply

ADR-0015 requires Apply to collect from the central staged map rather than mounted DOM (`documentation/docs/reference/decisions/0015-use-lazy-single-work-item-workbench-editing.md:63-71`). Instead, collection filters localStorage by the current prefix plus prefixes discovered from currently mounted `[data-storage-prefix]` elements (`crates/brain-brew-workbench-ui/src/lib.rs:3895-3921`; `crates/brain-brew-workbench-ui/src/staging.rs:82-105`). Stage in language A, switch to B, and A remains locally saved but is absent from B’s count, Preview, and Apply unless an A comparison pane happens to remain mounted. Current E2E covers item unmount within one scope and mounted secondary panes, not unmounted language/target/overlay scopes.

**Concrete test to add:** stage in A, switch to B so A is absent from the DOM, stage in B, then preview from B and assert both scopes/files appear and are applied—or explicitly design a draft manager that exposes and selects scopes.

#### P1 — Apply completion can reset the wrong selection and always ejects users to Notes

Every successful note-list publication forces `WorkbenchView::Notes` and clears all secondary view states (`crates/brain-brew-workbench-ui/src/lib.rs:2579-2608`). The post-Apply refresh therefore ejects a user from Card, Source String, or Metadata after their edit. More seriously, Apply does not capture a selection generation before posting. Its refresh captures the generation only after `/apply` returns (`:1665-1671,4018-4034`), so if a user changes language while Apply is pending, the old pivot refresh can be treated as current and switch the UI back to the old language. Existing E2E stops after seeing “Applied” and file bytes; it does not wait for and assert the settled view/selection (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:1240-1260`).

**Concrete tests to add:** delay `/apply`, switch language/view while it is pending, release it, and assert the latest language/view/selected item remain; after a normal Card/Source String/Metadata apply, wait for all refresh requests and assert the originating view and selected item remain active.

#### P1 — The comparison and Metadata workflows still violate selected-item scope

The comparison UI calls the legacy full `/comparison-pane`, hardcodes overlay `base`, and reuses the active target label (`crates/brain-brew-workbench-ui/src/lib.rs:1709-1732`). It then flattens every returned note field and renders every source-string/card summary (`:1813-1840,1871-1961`), recreating the all-items payload/DOM that ADR-0015 rejected. Metadata likewise calls the full `/metadata` endpoint and renders all editable items in one table (`:1524-1561,4094-4102`) even though the server already exposes `/metadata-list`. This is neither selected-item-scoped nor usable for target languages with different target labels/overlay groups.

**Concrete tests to add:** on UG, load a comparison pane and assert payload/DOM contain only the selected item and stay under a fixed row/media budget; select a non-`base` overlay and a language with a different target map and assert correct scope. Add `metadata-list` pagination plus one `metadata-detail` editor test.

#### P2 — Error/recovery and draft-management UX is incomplete

There is no Discard edit/Discard all control; `clear_prefix` is only called after Apply (`crates/brain-brew-workbench-ui/src/staging.rs:116-128`; `crates/brain-brew-workbench-ui/src/lib.rs:1665-1668`). A bad persistent draft therefore has no explicit UI recovery path. Async buttons are never disabled while requests run, and `post_json` does not inspect HTTP status or preserve plain-text API errors before trying JSON decoding (`:3873-3882`), so duplicate requests are possible and useful server validation messages can collapse into decode errors. New-language Create is enabled before Preview and changing Code/Template after Preview does not invalidate the preview (`:2000-2047,2050-2078`).

**Concrete tests to add:** invalid/duplicate new language with exact actionable error text and no writes; discard one/all drafts across refresh; failed Apply retains drafts; buttons expose busy/disabled state; changing scaffold inputs invalidates preview; repeated clicks issue one request.

#### P2 — Accessibility coverage and semantics are insufficient for a file-editing tool

Positive basics exist: visible focus outlines (`crates/brain-brew-workbench-ui/static/workbench.css:87-94`), labeled global selects, a labeled view nav, and inline preview fields receive textbox roles/labels. However:

- The persistent page heading is always “Deck Workbench Note pivot” even in other views (`crates/brain-brew-workbench-ui/src/lib.rs:366-369`).
- Advanced editor inputs/selects and Metadata/comparison inputs have no programmatic per-field labels; tables have no captions and their `<th>` cells omit `scope` (`:667-688,1095,1553-1560,1871-1918`).
- Apply output and new-language status are not live regions/status roles, and busy buttons expose neither `disabled` nor `aria-busy` (`:1685-1693,2070-2078`).
- Active Note/Card/Source String rows are visual-only; there is no `aria-current`/`aria-selected` state (`:562-586,1024-1036,1326-1335`).
- Browser tests use JavaScript to focus one inline field but do not test a keyboard-only tab/order/activation flow or accessible names (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:1791-1860`).

**Concrete tests to add:** automated accessibility scan on each loaded view plus semantic assertions for unique heading, labels/table headers, live apply status, active-row state, and busy state; a keyboard-only E2E that selects a work item, edits, previews, and confirms without script-forced focus or mouse clicks.

#### P2 — E2E remains happy-path biased and does not run the embedded release bundle

The suite is strong on happy paths but only one test checks severe browser logs, and no test asserts failed network responses, malformed JSON, server exit during startup, external-file conflicts, preview invalidation, Apply failure retaining drafts, pagination reachability, or accessibility. The E2E script always passes debug Trunk output through `--dev-assets` (`devenv.nix:60-67`); CLI tests verify embedded JS/WASM HTTP bytes/content types, and the freshness check proves byte equality, but no browser write flow boots the release-embedded bundle. The harness README is also stale: it still says Iced and claims full UG coverage is future work even though that test exists (`crates/brain-brew-workbench-e2e/README.md:9-14`).

**Concrete tests to add:** parameterize one complete edit/preview/confirm smoke to run without `BRAINBREW_E2E_DEV_ASSETS`; check severe console/network errors in every test teardown; add the failure/race tests listed above. Add a bounded timeout while waiting for the server’s first stdout line (`crates/brain-brew-workbench-e2e/tests/workbench_smoke.rs:2452-2470`).

#### P2 — Accepted ADR and documentation drift after the Leptos migration

ADR-0011 remains Accepted and explicitly chooses Iced/WASM (`documentation/docs/reference/decisions/0011-use-a-local-deck-workbench-server-with-iced-wasm-ui.md:1-19,28-35`), while the current crate and user documentation use Leptos CSR (`crates/brain-brew-workbench-ui/Cargo.toml:9,19-24`; `documentation/docs/reference/workbench.md:13-31`). The E2E README still says Iced (`crates/brain-brew-workbench-e2e/README.md:9`). The reference page also describes the old `note-pivot` API even though the UI uses `note-list` + `note-detail` (`documentation/docs/reference/workbench.md:50-56`; `crates/brain-brew-workbench-ui/src/lib.rs:4037-4062`). Supersede/update the ADR and synchronize docs; do not leave architecture rationale only in archived task records.

### Review gate

**Fail.** The browser suite is green and the embedded asset pipeline is fresh, but stale-file overwrite protection, incorrect progress, unreachable items, Preview bypass, incomplete staged-scope collection, and Apply selection races prevent treating the current UI as a safe complete real-user workflow.

```acceptance-report
{
  "criteriaSatisfied": [
    {
      "id": "criterion-1",
      "status": "satisfied",
      "evidence": "Review-only scope was preserved; only audit/09-workbench-ui-e2e.md was added and no product source or tests were edited."
    },
    {
      "id": "criterion-2",
      "status": "satisfied",
      "evidence": "The report maps the user states, cites exact UI/E2E/ADR locations, records focused validation commands, ranks verified findings, and specifies concrete browser/API tests for each major gap."
    }
  ],
  "changedFiles": [
    "audit/09-workbench-ui-e2e.md"
  ],
  "testsAddedOrUpdated": [],
  "commandsRun": [
    {
      "command": "jj status && jj diff --stat",
      "result": "passed",
      "summary": "Inspected the Jujutsu working copy; only audit reports from concurrent review work were present, with no product-source changes from this reviewer."
    },
    {
      "command": "devenv shell cargo test -p brain-brew-workbench-ui",
      "result": "passed",
      "summary": "1 integration test passed; the host-side UI crate currently has no behavior tests beyond WorkspaceSummary."
    },
    {
      "command": "devenv shell e2e",
      "result": "passed",
      "summary": "All 13 real Chromium/ChromeDriver Workbench tests passed in 22.02s."
    },
    {
      "command": "devenv shell workbench-ui-embed-check",
      "result": "passed",
      "summary": "Release Trunk output matched checked-in embedded JS/WASM/CSS/index assets exactly."
    }
  ],
  "validationOutput": [
    "UI tests: 1 passed, 0 failed.",
    "Browser E2E: 13 passed, 0 failed, finished in 22.02s.",
    "Embedded asset check: Workbench embedded release assets are fresh.",
    "Review gate: failed because verified workflow/ADR gaps remain despite green happy-path tests."
  ],
  "residualRisks": [
    "No product code was changed, so all ranked findings remain open.",
    "The requested plan.md and progress.md were absent and could not be audited.",
    "No automated accessibility scanner was installed; accessibility findings are based on DOM semantics and existing test inspection.",
    "The E2E run exercised dev assets, not a browser session booted from the embedded release bundle."
  ],
  "noStagedFiles": true,
  "notes": "Jujutsu has no Git staging area and no staging/commit operation was performed. Other audit markdown files were concurrently present in the working copy; this reviewer added only audit/09-workbench-ui-e2e.md."
}
```
